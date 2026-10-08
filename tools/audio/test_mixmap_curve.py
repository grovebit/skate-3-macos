import struct
import unittest

from tools.owned_game.mixmap_curve import evaluate_curve, evaluate_float_curve, read_curve_table


class MixMapCurveTests(unittest.TestCase):
    # Synthetic table, deliberately nonmonotonic to exercise both delta signs.
    table = tuple((i * 197) % 32768 for i in range(513))

    def test_direct_zero_and_last_bin_shortcuts(self):
        self.assertEqual(evaluate_curve(0, 0, self.table), 0)
        self.assertEqual(evaluate_curve(511 * 64, 0, self.table), 0)
        self.assertEqual(evaluate_curve(32767, 0, self.table), 0)
        self.assertEqual(evaluate_curve(64, 0, self.table), 203)

    def test_interpolation_ignores_low_bit_and_bit_five(self):
        for value in [64, 65, 96, 97]:
            self.assertEqual(evaluate_curve(value, 0, self.table), 203)
        for value in [66, 67, 98, 99]:
            self.assertEqual(evaluate_curve(value, 0, self.table), 209)
        self.assertEqual(evaluate_curve(94, 0, self.table), 295)

    def test_negative_delta_rounds_down(self):
        # Entries 166 and 167 straddle the synthetic table's wrap.
        self.assertEqual(evaluate_curve(166 * 64, 0, self.table), 31685)

    def test_reverse_complement_and_square(self):
        self.assertEqual(evaluate_curve(0, 4, self.table), 30397)
        self.assertEqual(evaluate_curve(0, 6, self.table), 28197)
        for value in [0, 1, 64, 16384, 32767]:
            for selector in [1, 3, 5, 7, 9]:
                self.assertEqual(evaluate_curve(value, selector, self.table),
                                 evaluate_curve(32767 - value, selector - 1, self.table))
            self.assertEqual(evaluate_curve(value, 8, self.table), 32767 - value)
            self.assertEqual(evaluate_curve(value, 9, self.table), value)

    def test_table_and_input_validation(self):
        raw = struct.pack('>513i', *self.table)
        self.assertEqual(read_curve_table(raw), self.table)
        for data in [b'', raw[:-1], raw + b'\0']:
            with self.assertRaises(ValueError):
                read_curve_table(data)
        for value, selector in [(-1, 0), (32768, 0), (0, -1), (0, 10)]:
            with self.assertRaises(ValueError):
                evaluate_curve(value, selector, self.table)
        for table in [self.table[:-1], [-1] * 513, [32768] * 513]:
            with self.assertRaises(ValueError):
                evaluate_curve(0, 0, table)

    def test_float_wrapper_quantizes_before_scaling_back(self):
        # Selector 9 is identity only in the integer domain. Half becomes
        # 16383, then returns the f32 product with original 0x38000100.
        for value, bits in [(0.0, '00000000'), (1.0, '3f800000'),
                            (0.5, '3efffe00'), (0.1, '3dccc19a')]:
            result = evaluate_float_curve(value, 9, self.table)
            self.assertEqual(struct.pack('>f', result).hex(), bits)

    def test_float_wrapper_rounds_multiply_before_truncation(self):
        # This binary32 input times 32767 is just below 16384 in binary64,
        # but native FMULS rounds it to 16384 before FCTIWZ truncates it.
        value = struct.unpack('>f', bytes.fromhex('3f000100'))[0]
        self.assertEqual(int(value * 32767), 16383)
        result = evaluate_float_curve(value, 9, self.table)
        self.assertEqual(struct.pack('>f', result).hex(), '3f000100')

    def test_float_wrapper_domain_and_selector_bypass(self):
        for value in [-0.01, 1.01, float('nan'), float('inf')]:
            with self.assertRaises(ValueError):
                evaluate_float_curve(value, 1, self.table)
        for selector in [10, 15, 0xFFFFFFFF]:
            self.assertEqual(evaluate_float_curve(float('nan'), selector, ()), 0.0)
        for selector in [-1, 0x100000000, 1.5]:
            with self.assertRaises(ValueError):
                evaluate_float_curve(0.5, selector, self.table)


if __name__ == '__main__':
    unittest.main()
