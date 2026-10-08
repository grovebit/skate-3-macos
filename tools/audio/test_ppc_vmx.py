import unittest

from tools.owned_game.ppc_vmx import decode


def registers(d, a, b):
    return ((d & 31) << 21) | ((d >> 5) << 2) | ((a & 31) << 16) | \
           (((a >> 5) & 1) << 5) | ((a >> 6) << 10) | ((b & 31) << 11) | (b >> 5)


class VmxTests(unittest.TestCase):
    def test_all_extended_register_bits(self):
        for d in [0, 31, 32, 63, 64, 95, 96, 127]:
            for a in [0, 31, 32, 63, 64, 95, 96, 127]:
                for b in [0, 31, 32, 63, 64, 95, 96, 127]:
                    with self.subTest(d=d, a=a, b=b):
                        self.assertEqual(decode(0x14000190 | registers(d, a, b)),
                                         ('vmsum3fp128', f'v{d}, v{a}, v{b}'))

    def test_memory_address_registers_do_not_use_vector_extensions(self):
        word = 0x100000C3 | (5 << 21) | (3 << 2) | (17 << 16) | (29 << 11)
        self.assertEqual(decode(word), ('lvx128', 'v101, r17, r29'))
        self.assertEqual(decode(word | 0x100), ('stvx128', 'v101, r17, r29'))

    def test_vector_shift_uses_extended_register_operands(self):
        self.assertEqual(decode(0x180000D0 | registers(59, 63, 63)),
                         ('vslw128', 'v59, v63, v63'))

    def test_immediates_and_unary_registers(self):
        word = 0x180002B0 | registers(72, 3, 105)
        self.assertEqual(decode(word), ('vcsxwfp128', 'v72, v105, 3'))
        self.assertEqual(decode(0x18000670 | registers(72, 0, 105)),
                         ('vrsqrtefp128', 'v72, v105'))
        for raw, signed in [(0, 0), (15, 15), (16, -16), (31, -1)]:
            self.assertEqual(decode(0x18000770 | (raw << 16) | (3 << 2)),
                             ('vspltisw128', f'v96, {signed}'))

    def test_compare_record_bit_and_permutation_fields(self):
        word = 0x18000080 | registers(111, 82, 53)
        self.assertEqual(decode(word), ('vcmpgefp128', 'v111, v82, v53'))
        self.assertEqual(decode(word | 0x40), ('vcmpgefp128.', 'v111, v82, v53'))
        self.assertEqual(decode(0x14000000 | registers(111, 82, 53) | (5 << 6)),
                         ('vperm128', 'v111, v82, v53, v5'))
        for perm in [0, 31, 32, 127, 255]:
            word = 0x18000210 | registers(111, perm & 31, 53) | ((perm >> 5) << 6)
            self.assertEqual(decode(word), ('vpermwi128', f'v111, v53, {perm:#04x}'))

    def test_does_not_claim_standard_or_unknown_instructions(self):
        for word in [0, 0x4E800020, 0x1000002A, 0x1000004A, 0xFC000000]:
            self.assertIsNone(decode(word))
        for word in [-1, 1 << 32]:
            with self.assertRaises(ValueError):
                decode(word)


if __name__ == '__main__':
    unittest.main()
