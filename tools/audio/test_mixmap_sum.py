import unittest

from tools.owned_game.mixmap_sum import evaluate_sum, expand_references


class MixMapSumTests(unittest.TestCase):
    def test_binding_replaces_authored_instance_bits_and_preserves_order(self):
        refs = [0x30033000, 0xB100304C, 0xB1013014, 0xB100304C]
        self.assertEqual(expand_references(refs, 3, 2, {0: 2, 1: 0}),
                         (0x30031000, 0xB100004C, 0xB100084C,
                          0xB100004C, 0xB100084C))

    def test_binding_requires_explicit_external_counts(self):
        self.assertEqual(expand_references([0x30033000], 3, 31, {}), (0x3003F800,))
        for counts in [{}, {0: -1}, {0: 33}]:
            with self.assertRaises(ValueError):
                expand_references([0xB100304C], 3, 0, counts)

    def test_collision_sum_clamp_and_word_overflow(self):
        self.assertEqual(evaluate_sum([-100, -200], 0x0000D8F0), -300)
        self.assertEqual(evaluate_sum([-9000, -9000], 0x0000D8F0), -10000)
        self.assertEqual(evaluate_sum([100], 0x0000D8F0), 0)
        self.assertEqual(evaluate_sum([0x7FFFFFFF, 1], 0x0000D8F0), -10000)
        self.assertEqual(evaluate_sum([-0x80000000, -1], 0x0000D8F0), 0)

    def test_lower_bound_is_forced_negative_not_sign_extended(self):
        self.assertEqual(evaluate_sum([-100000], 0x800A0001), -65535)
        self.assertEqual(evaluate_sum([100], 0x800A0001), 10)

    def test_null_array_retains_state_but_empty_array_recomputes(self):
        self.assertEqual(evaluate_sum(None, None, previous=-321), -321)
        self.assertEqual(evaluate_sum([], 0x0000D8F0, previous=-321), 0)

    def test_execution_uses_only_low_byte_of_expanded_count(self):
        self.assertEqual(evaluate_sum([-1] * 255, 0x0000D8F0), -255)
        self.assertEqual(evaluate_sum([-1] * 256, 0x0000D8F0), 0)
        self.assertEqual(evaluate_sum([-123] + [-1] * 256, 0x0000D8F0), -123)


if __name__ == '__main__':
    unittest.main()
