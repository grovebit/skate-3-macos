import struct
import unittest

from tools.owned_game.mixmap_volume import format0_volume, read_volume_table


class MixMapVolumeTests(unittest.TestCase):
    # Synthetic descending lookup addresses, not copied original table values.
    table = tuple(16000 + 20 * i for i in range(602))

    def test_lookup_direction_and_shift_boundaries(self):
        for level, index, shift in [(0, 601, 0), (-1, 600, 0), (-601, 0, 0),
                                    (-602, 601, 1), (-603, 600, 1),
                                    (-9030, 601, 15), (-9631, 0, 15)]:
            with self.subTest(level=level):
                result = format0_volume(level, 0, self.table)
                self.assertEqual((result['table_index'], result['shift']), (index, shift))
                self.assertEqual(result['packed_halfword'], self.table[index] >> shift)

    def test_clamp_and_silence_cutoff(self):
        self.assertEqual(format0_volume(100, 0, self.table)['packed_halfword'],
                         self.table[601])
        for level, clamped in [(-9632, -9632), (-10000, -10000), (-20000, -10000)]:
            with self.subTest(level=level):
                result = format0_volume(level, 0, self.table)
                self.assertEqual(result['clamped_level'], clamped)
                self.assertIsNone(result['table_index'])
                self.assertEqual(result['packed_halfword'], 0)

    def test_adds_all_operands_before_clamping(self):
        result = format0_volume(100, -123, self.table, -579)
        self.assertEqual(result['summed_level'], -602)
        self.assertEqual(result['packed_halfword'], self.table[601] >> 1)
        self.assertEqual(format0_volume(-602, 0, self.table),
                         format0_volume(-602, 0, self.table, 0))

    def test_preserves_signed_word_wraparound(self):
        self.assertEqual(format0_volume(0x7FFFFFFF, 1, self.table)['summed_level'],
                         -0x80000000)
        self.assertEqual(format0_volume(-0x80000000, -1, self.table)['summed_level'],
                         0x7FFFFFFF)

    def test_arithmetic_shift_then_low_halfword_write(self):
        table = [-3] * 602
        self.assertEqual(format0_volume(-602, 0, table)['packed_halfword'], 0xFFFE)

    def test_table_reader_and_invalid_inputs(self):
        raw = struct.pack('>602i', *self.table)
        self.assertEqual(read_volume_table(raw), self.table)
        for data in [b'', raw[:-1], raw + b'\0']:
            with self.assertRaises(ValueError):
                read_volume_table(data)
        for level, adjustment, modulation in [(1 << 31, 0, None),
                                               (0, 32768, None), (0, -32769, None),
                                               (0, 0, -(1 << 31) - 1)]:
            with self.assertRaises(ValueError):
                format0_volume(level, adjustment, self.table, modulation)
        for table in [self.table[:-1], [1 << 31] * 602]:
            with self.assertRaises(ValueError):
                format0_volume(0, 0, table)


if __name__ == '__main__':
    unittest.main()
