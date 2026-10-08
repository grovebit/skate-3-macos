import copy
import struct
import unittest

from tools.owned_game.body_controls import body_controls


def fixture():
    return [{'class': 'aud_material', 'key': name, 'fields': {
        'Hash_875BA75341DC8391': {'type': 'EA::Reflection::Int32', 'data': '00004650'},
        'Hash_C090F2C1F048F17B': {'type': 'EA::Reflection::Int32', 'data': '00000ED4'},
        'Hash_D5EF686287A57AFE': {'type': 'Sk8::Audio::eMaterialNicotineType', 'data': '00000005'},
    }} for name in ['head', 'torso', 'arm', 'leg']]


class BodyControlsTests(unittest.TestCase):
    def test_preserves_authored_integers_and_selects_volume_slot(self):
        result = body_controls(fixture())['materials']
        self.assertEqual(result['head'], {'authored_base_level': 18000, 'base_level': 18000,
                         'pitch': 3796, 'material_class': 5, 'volume_controller_slot': 18})
        self.assertEqual(len(result), 4)

    def test_clamps_base_level_but_retains_source_value(self):
        for authored, expected in [(-1, 0), (0, 0), (32767, 32767), (40000, 32767)]:
            rows = fixture()
            rows[0]['fields']['Hash_875BA75341DC8391']['data'] = struct.pack('>i', authored).hex()
            result = body_controls(rows)['materials']['head']
            self.assertEqual(result['base_level'], expected)
            self.assertEqual(result['authored_base_level'], authored)

    def test_controller_class_mapping_including_unsigned_default_branch(self):
        for kind, slot in enumerate([13, 14, 15, 16, 17, 18, 12, 19, 20, 21]):
            rows = fixture()
            rows[0]['fields']['Hash_D5EF686287A57AFE']['data'] = struct.pack('>i', kind).hex()
            self.assertEqual(body_controls(rows)['materials']['head']['volume_controller_slot'], slot)
        for kind in [-1, 10, 2147483647]:
            rows = fixture()
            rows[0]['fields']['Hash_D5EF686287A57AFE']['data'] = struct.pack('>i', kind).hex()
            self.assertEqual(body_controls(rows)['materials']['head']['volume_controller_slot'], 20)

    def test_rejects_wrong_type_size_missing_and_duplicate_materials(self):
        for value in [{'type': 'EA::Reflection::Float', 'data': '3F800000'},
                      {'type': 'EA::Reflection::Int32', 'data': '0000'}]:
            rows = fixture()
            rows[0]['fields']['Hash_875BA75341DC8391'] = value
            with self.assertRaises(ValueError):
                body_controls(rows)
        rows = fixture()
        for bad in [rows[:-1], rows + [copy.deepcopy(rows[0])]]:
            with self.assertRaises(ValueError):
                body_controls(bad)


if __name__ == '__main__':
    unittest.main()
