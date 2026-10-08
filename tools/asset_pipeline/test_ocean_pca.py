"""Ocean PCA extraction from synthetic mapped images; no game content."""
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

from tools.owned_game import xex
from . import ocean_pca


def image(means, weights):
    address, size, _ = ocean_pca.TABLE
    offset = address - xex.IMAGE_BASE
    data = bytearray(offset + size)
    struct.pack_into(f'>{len(means) + len(weights)}f', data, offset, *means, *weights)
    return data, hashlib.sha256(data[offset:]).hexdigest()


class OceanPcaTests(unittest.TestCase):
    def setUp(self):
        # Distinct binary32 values: frame and component order are both visible.
        self.means = [i + 0.5 for i in range(ocean_pca.FRAMES * 3)]
        self.weights = [i * 0.25 - 300.0 for i in range(ocean_pca.FRAMES * 24)]
        self.data, digest = image(self.means, self.weights)
        self.table = (ocean_pca.TABLE[0], ocean_pca.TABLE[1], digest)

    def test_layout_matches_the_two_pointers_formed_by_init(self):
        address, size, _ = ocean_pca.TABLE
        self.assertEqual(address, 0x82FC3978)
        self.assertEqual(address + ocean_pca.FRAMES * 3 * 4, 0x82FC3AE0)
        self.assertEqual(size, (0x82FC3AE0 - address) + ocean_pca.FRAMES * 24 * 4)
        self.assertEqual(struct.pack('>f', ocean_pca.SCALE).hex(), '3b808081')

    def test_frames_keep_the_native_constant_order_and_single_precision(self):
        with mock.patch.object(ocean_pca, 'TABLE', self.table):
            frames = ocean_pca.frames(bytes(self.data))
        self.assertEqual(len(frames), 30)
        scale = lambda v: ocean_pca.single(v * ocean_pca.SCALE)
        for frame, rows in enumerate(frames):
            self.assertEqual(rows[0], [scale(v) for v in self.means[frame * 3:frame * 3 + 3]] + [1.0])
            # R_0, R_1, G_0, G_1, B_0, B_1: consecutive groups of four weights.
            for row in range(6):
                start = frame * 24 + row * 4
                self.assertEqual(rows[row + 1], [scale(v) for v in self.weights[start:start + 4]])
        for value in (v for rows in frames for row in rows for v in row):
            self.assertEqual(ocean_pca.single(value), value)
        # fmuls rounding, not a double-precision division by 255.
        value = ocean_pca.single(1.1)
        self.assertEqual(scale(value), struct.unpack('>f', struct.pack('>f', value * ocean_pca.SCALE))[0])
        self.assertNotEqual(scale(value), value / 255)

    def test_executable_and_table_are_verified(self):
        executable = b'synthetic default.xex'
        with tempfile.TemporaryDirectory() as temp, \
                mock.patch.object(ocean_pca, 'TABLE', self.table), \
                mock.patch.object(xex, 'XEX_SHA256', hashlib.sha256(executable).hexdigest()), \
                mock.patch.object(xex, 'IMAGE_SHA256', hashlib.sha256(self.data).hexdigest()), \
                mock.patch.object(xex, 'extract', return_value=bytes(self.data)) as extract:
            game = Path(temp)
            (game / 'default.xex').write_bytes(executable)
            assets = game / 'assets'
            self.assertEqual(ocean_pca.convert(game, assets), 30)
            extract.assert_called_once_with(executable)
            written = json.loads((assets / 'private/ocean-pca.json').read_text())
            self.assertEqual(set(written), {'source_sha256', 'frames'})
            self.assertEqual(written['source_sha256'], xex.XEX_SHA256)
            self.assertEqual(written['frames'], ocean_pca.frames(bytes(self.data)))
            # The table hash is checked even inside a verified image.
            changed = bytearray(self.data)
            changed[-1] ^= 1
            with mock.patch.object(xex, 'IMAGE_SHA256', hashlib.sha256(changed).hexdigest()):
                extract.return_value = bytes(changed)
                with self.assertRaisesRegex(ValueError, 'Ocean PCA'):
                    ocean_pca.convert(game, assets)
            (game / 'default.xex').write_bytes(b'another build')
            with self.assertRaisesRegex(ValueError, 'verified base-disc'):
                ocean_pca.convert(game, assets)


if __name__ == '__main__':
    unittest.main()
