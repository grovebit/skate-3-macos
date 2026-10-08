import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

from tools.owned_game import collision_mix


def rows(**overrides):
    values = {'camera': ('Hash_A6F853B935E46E5F', 0.25),
              'treatments': ('Hash_A12258EB71B6937A', 1.0),
              'hom': ('Hash_4BD3E0CCEA90BE26', 1.0)}
    values.update(overrides)
    return [{'class': 'aud_general', 'key': key,
             'fields': {field: {'type': 'EA::Reflection::Float',
                                'data': struct.pack('>f', value).hex().upper()}}}
            for key, (field, value) in values.items()]


class CollisionMixTests(unittest.TestCase):
    def test_settings_read_typed_authored_floats(self):
        self.assertEqual(collision_mix.settings(rows()),
                         {'listener_offset': 0.25, 'treatment_scale': 1.0, 'hom_threshold': 1.0})
        broken = rows()
        broken[0]['fields']['Hash_A6F853B935E46E5F']['type'] = 'EA::Reflection::Int32'
        with self.assertRaises(ValueError):
            collision_mix.settings(broken)
        with self.assertRaises(ValueError):
            collision_mix.settings(rows()[1:])
        with self.assertRaises(ValueError):
            collision_mix.settings(rows(camera=('Hash_A6F853B935E46E5F', float('nan'))))

    def test_tables_are_sliced_from_the_mapped_image_and_hash_checked(self):
        image = bytearray(0x1000000)
        tables = {}
        for name, (address, size, _) in collision_mix.TABLES.items():
            blob = bytes((i * 7 + len(name)) & 255 for i in range(size))
            offset = address - collision_mix.IMAGE_BASE
            image[offset:offset + size] = blob
            tables[name] = (address, size, hashlib.sha256(blob).hexdigest())
        xex = b'synthetic'
        with mock.patch.object(collision_mix, 'TABLES', tables), \
                mock.patch.object(collision_mix, 'XEX_SHA256', hashlib.sha256(xex).hexdigest()):
            result = collision_mix.executable_tables(xex, lambda raw: bytes(image))
            self.assertEqual({k: len(v) for k, v in result.items()},
                             {k: v[1] for k, v in tables.items()})
            image[tables['log'][0] - collision_mix.IMAGE_BASE] ^= 1
            with self.assertRaises(ValueError):
                collision_mix.executable_tables(xex, lambda raw: bytes(image))
            with self.assertRaises(ValueError):
                collision_mix.executable_tables(b'other', lambda raw: bytes(image))

    def test_inputs_verify_the_program_and_write_never_overwrites(self):
        program = b'program'
        xex = b'synthetic'
        image = bytes(0x10)
        with tempfile.TemporaryDirectory() as temp, \
                mock.patch.object(collision_mix, 'PROGRAM',
                                  ('MixMapSK8.mxb', hashlib.sha256(program).hexdigest())), \
                mock.patch.object(collision_mix, 'TABLES',
                                  {'log': (collision_mix.IMAGE_BASE + 4, 2,
                                           hashlib.sha256(bytes(2)).hexdigest())}), \
                mock.patch.object(collision_mix, 'XEX_SHA256', hashlib.sha256(xex).hexdigest()):
            game = Path(temp) / 'game'
            (game / 'data/audio').mkdir(parents=True)
            (game / 'data/audio/MixMapSK8.mxb').write_bytes(program)
            (game / 'default.xex').write_bytes(xex)
            inputs = collision_mix.inputs(game, rows(), lambda raw: image)
            self.assertEqual(inputs, (program, {'log': bytes(2)},
                                      {'listener_offset': 0.25, 'treatment_scale': 1.0,
                                       'hom_threshold': 1.0}))
            output = Path(temp) / 'mixmap'
            manifest = collision_mix.write(output, *inputs, [])
            stored = json.loads((output / 'mixmap.json').read_text())
            self.assertEqual(stored, manifest)
            self.assertEqual(stored['files'], {'program': 'MixMapSK8.mxb', 'log': 'log.bin'})
            self.assertEqual((output / 'log.bin').read_bytes(), bytes(2))
            with self.assertRaises(FileExistsError):
                collision_mix.write(output, *inputs, [])
            (game / 'data/audio/MixMapSK8.mxb').write_bytes(b'changed')
            with self.assertRaises(ValueError):
                collision_mix.inputs(game, rows(), lambda raw: image)

if __name__ == '__main__':
    unittest.main()
