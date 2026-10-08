import unittest

from tools.audio.check_collision_voice import binding_key, collision_gain, controller_level


class CollisionVoiceTests(unittest.TestCase):
    def test_binding_key_decodes_collision_family_group_and_object(self):
        # Independently assembled fixture: family3, group17, object85, low flags9.
        result = binding_key(0x40038D59)
        self.assertEqual(result, {'family': 3, 'manager_slot_offset': 0x28C,
                                 'group': 17, 'object': 85, 'object_list_offset': 0x24})
        self.assertEqual(binding_key(0x5F038D50), result)

    def test_binding_list_requires_exact_top_three_bit_pattern(self):
        for tag in range(8):
            key = binding_key((tag << 29) | 0x00030000)
            self.assertEqual(key['object_list_offset'], 0x24 if tag == 2 else 0x20)
        self.assertEqual(binding_key(0xFFFFFFFF)['family'], 255)
        self.assertEqual(binding_key(0xFFFFFFFF)['group'], 31)
        self.assertEqual(binding_key(0xFFFFFFFF)['object'], 127)
        for invalid in [-1, 1 << 32]:
            with self.assertRaises(ValueError):
                binding_key(invalid)

    def test_packed_words_use_numeric_halfword_order_and_mask_flag_bits(self):
        words = [0xFEDC9234, 0x80000001]
        self.assertEqual([controller_level(words, i) for i in range(4)],
                         [0x1234, 0x7EDC, 1, 0])

    def test_missing_controller_is_zero_but_truncated_capture_is_an_error(self):
        self.assertEqual(controller_level(None, 18), 0)
        with self.assertRaises(ValueError):
            controller_level([], 18)
        for words, slot in [([0], -1), ([-1], 0), ([0x100000000], 0)]:
            with self.assertRaises(ValueError):
                controller_level(words, slot)

    def test_two_integer_truncations_differ_from_combining_the_factors(self):
        result = collision_gain(10002, 18000, [12345], 0)
        self.assertEqual(result['after_controller'], 3768)
        self.assertEqual(result['after_material'], 2069)
        self.assertEqual(result['voice_gain'], 0.0631427988409996)
        # A single truncation would produce 2070 for these same inputs.
        self.assertEqual(int(10002 * 12345 / 32767 * 18000 / 32767), 2070)

    def test_full_scale_zero_and_authored_base_clamping(self):
        self.assertEqual(collision_gain(32767, 32767, [32767], 0)['voice_gain'], 1.)
        for base in [-1, 0]:
            self.assertEqual(collision_gain(32767, base, [32767], 0)['voice_gain'], 0.)
        self.assertEqual(collision_gain(32767, 40000, [32767], 0)['voice_gain'], 1.)
        self.assertEqual(collision_gain(32767, 32767, None, 18)['voice_gain'], 0.)
        self.assertEqual(collision_gain(0, 32767, [32767], 0)['voice_gain'], 0.)

    def test_truncation_is_toward_zero_and_invalid_integer_domains_fail(self):
        result = collision_gain(-10002, 18000, [12345], 0)
        self.assertEqual(result['after_controller'], -3768)
        self.assertEqual(result['after_material'], -2069)
        for queued, base in [(1 << 31, 32767), (0, -(1 << 31) - 1)]:
            with self.assertRaises(ValueError):
                collision_gain(queued, base, [32767], 0)


if __name__ == '__main__':
    unittest.main()
