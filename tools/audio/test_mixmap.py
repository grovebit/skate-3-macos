import struct
import unittest

from tools.owned_game.mixmap import declarations, modulation_routes, output_routes


def fixture():
    data = bytearray(112)
    struct.pack_into('>4I', data, 0, 0, 2, 16, 0xFFFFFFFF)
    struct.pack_into('>2I', data, 16, 32, 0xFFFFFFFF)
    struct.pack_into('>2I', data, 32, 0x001F0000, 16)
    struct.pack_into('>4I', data, 48, 2, 0, 0, 0)
    # First record has two arguments, then a second record with no arguments.
    struct.pack_into('>6I', data, 64, 0x98030000, 0x00021234,
                     0x400000C2, 0xABCDEF12, 0x90030000, 0x0000FE70)
    return data


class MixMapTests(unittest.TestCase):
    def test_relative_sections_and_variable_arguments_preserve_raw_words(self):
        result = declarations(fixture())
        self.assertEqual(result['directory_offset'], 16)
        group = result['groups'][0]
        self.assertEqual(group['offset'], 32)
        self.assertEqual(group['section_offset'], 48)
        self.assertEqual(group['declarations'], [
            {'offset': 64, 'id': '98030000', 'descriptor': '00021234',
             'arguments': ['400000C2', 'ABCDEF12']},
            {'offset': 80, 'id': '90030000', 'descriptor': '0000FE70', 'arguments': []},
        ])
        self.assertIsNone(result['groups'][1]['offset'])

    def test_negative_section_offsets_are_absent_without_guessing_defaults(self):
        for relative in [0xFFFFFFFF, 0xFFFFFFFE]:
            data = fixture()
            struct.pack_into('>I', data, 36, relative)
            group = declarations(data)['groups'][0]
            self.assertIsNone(group['section_offset'])
            self.assertEqual(group['declarations'], [])

    def test_argument_count_uses_only_five_bits(self):
        data = fixture()
        struct.pack_into('>I', data, 68, 0xFFE21234)
        records = declarations(data)['groups'][0]['declarations']
        self.assertEqual(len(records[0]['arguments']), 2)
        self.assertEqual(records[1]['offset'], 80)

    def test_does_not_require_disjoint_directory_and_unused_group_word(self):
        data = fixture()
        struct.pack_into('>I', data, 16, 20)
        # Group +00 shares the final directory word, as in the owned file.
        struct.pack_into('>I', data, 24, 28)
        group = declarations(data)['groups'][0]
        self.assertEqual(group['section_offset'], 48)
        self.assertEqual(len(group['declarations']), 2)

    def test_rejects_truncated_headers_records_and_arguments(self):
        for size in [0, 12, 19, 37, 60, 70, 76, 83]:
            with self.subTest(size=size), self.assertRaises(ValueError):
                declarations(fixture()[:size])

    def test_rejects_invalid_counts_offsets_and_alignment(self):
        for offset, value in [(4, 0xFFFFFFFF), (4, 0x7FFFFFFF),
                              (8, 0xFFFFFFFC), (16, 33), (36, 0x7FFFFFFC),
                              (48, 0xFFFFFFFF), (48, 0x7FFFFFFF)]:
            with self.subTest(offset=offset, value=value), self.assertRaises(ValueError):
                data = fixture()
                struct.pack_into('>I', data, offset, value)
                declarations(data)


def output_fixture():
    data = bytearray(160)
    struct.pack_into('>4I', data, 0, 0, 2, 16, 0xFFFFFFFF)
    struct.pack_into('>2I', data, 16, 32, 0xFFFFFFFF)
    struct.pack_into('>6I', data, 32, 0, 0xFFFFFFFF, 0, 0, 32, 96)
    struct.pack_into('>4I', data, 64, 2, 1, 0, 0)
    struct.pack_into('>5I', data, 80, 0xC0020300, 0x0000D8F0, 0x40030000,
                     0x90030000, 0x30030000)
    struct.pack_into('>3I', data, 100, 0xC1000301, 0, 0x40030000)
    struct.pack_into('>5I', data, 128, 0xE0030002, (7 << 26) | (1 << 21) | 0xFF85,
                     (8 << 26) | 0x7FFF, 0xE1030101, (1 << 26))
    return data


class MixMapOutputTests(unittest.TestCase):
    def test_output_records_pair_with_variable_write_maps(self):
        groups = output_routes(output_fixture())
        group = groups[0]
        self.assertEqual((group['section_offset'], group['mapping_offset'],
                          group['buffer_count']), (64, 128, 1))
        first, second = group['routes']
        self.assertEqual(first['arguments'], ['90030000', '30030000'])
        self.assertEqual(first['controller_id'], '40030000')
        self.assertEqual(first['format'], 0)
        self.assertEqual(first['write_count'], 2)
        self.assertEqual(first['writes'][0], {'word': '1C20FF85', 'slot': 7,
                                             'argument_index': 1, 'low_i16': -123})
        self.assertEqual(first['writes'][1]['low_i16'], 32767)
        self.assertEqual((second['offset'], second['mapping_offset'], second['format']),
                         (100, 140, 1))
        self.assertEqual(second['arguments'], [])
        self.assertEqual(groups[1]['routes'], [])

    def test_traversal_and_execution_count_masks_are_distinct(self):
        data = output_fixture() + bytearray(128)
        struct.pack_into('>I', data, 128, 0xE0030022)
        struct.pack_into('>2I', data, 268, 0xE1030101, 1 << 26)
        first, second = output_routes(data)[0]['routes']
        self.assertEqual(len(first['writes']), 34)
        self.assertEqual(first['write_count'], 2)
        self.assertEqual(second['mapping_offset'], 268)

    def test_absent_or_empty_outputs_do_not_read_mapping(self):
        for field, value in [(48, 0xFFFFFFFF), (64, 0)]:
            data = output_fixture()
            struct.pack_into('>I', data, field, value)
            struct.pack_into('>I', data, 52, 0xFFFFFFFF)
            self.assertEqual(output_routes(data)[0]['routes'], [])

    def test_rejects_missing_truncated_or_misaligned_output_data(self):
        for size in [52, 70, 91, 110, 129, 145]:
            with self.subTest(size=size), self.assertRaises(ValueError):
                output_routes(output_fixture()[:size])
        for offset, value in [(48, 33), (52, 0xFFFFFFFF), (52, 97),
                              (64, 0xFFFFFFFF), (64, 0x7FFFFFFF),
                              (80, 0xC0FF0300), (128, 0xE00300FF)]:
            with self.subTest(offset=offset, value=value), self.assertRaises(ValueError):
                data = output_fixture()
                struct.pack_into('>I', data, offset, value)
                output_routes(data)


def modulation_fixture():
    data = bytearray(160)
    struct.pack_into('>4I', data, 0, 0, 2, 16, 0xFFFFFFFF)
    struct.pack_into('>2I', data, 16, 32, 0xFFFFFFFF)
    struct.pack_into('>3I', data, 32, 0, 0xFFFFFFFF, 32)
    struct.pack_into('>4I', data, 64, 2, 0, 0, 0)
    struct.pack_into('>I', data, 80, 0x82050010)
    struct.pack_into('>6I', data, 84, 0x90050000, 0x12345678,
                     0x800AFFFB, 0x00140002, 0x001E0003, 0x00280004)
    struct.pack_into('>6I', data, 108, 0x91051100, 0x87654321, 1, 2, 3, 4)
    struct.pack_into('>7I', data, 132, 0x81050020, 0x90052200, 0, 1, 2, 3, 4)
    return data


class MixMapModulationTests(unittest.TestCase):
    def test_modes_stride_and_input_binding(self):
        groups = modulation_routes(modulation_fixture())
        first, second = groups[0]['nodes']
        self.assertEqual(first['controller_id'], '60050010')
        self.assertEqual(second['offset'], 132)
        self.assertEqual(second['controller_id'], '60050020')
        mode0, mode1 = first['modes']
        self.assertEqual((mode0['mode'], mode1['mode']), (0, 1))
        self.assertEqual((mode0['float_input_slot'], mode0['phase_input_slot']), (1, 3))
        self.assertEqual((mode1['float_input_slot'], mode1['phase_input_slot']), (0, 2))
        self.assertEqual(mode0['bounds_words'][0], '800AFFFB')
        self.assertEqual(mode0['bounds'][0], [32763, 10])
        self.assertIsNone(second['modes'][0]['float_input_slot'])
        self.assertIsNone(second['modes'][0]['phase_input_slot'])
        self.assertEqual(groups[1]['nodes'], [])

    def test_negative_section_and_zero_modes(self):
        data = modulation_fixture()
        struct.pack_into('>I', data, 40, 0xFFFFFFFE)
        self.assertEqual(modulation_routes(data)[0]['nodes'], [])
        data = modulation_fixture()
        struct.pack_into('>I', data, 64, 1)
        struct.pack_into('>I', data, 80, 0x80050010)
        self.assertEqual(modulation_routes(data)[0]['nodes'][0]['modes'], [])

    def test_rejects_invalid_ranges(self):
        for size in [40, 68, 83, 103, 155, 159]:
            with self.subTest(size=size), self.assertRaises(ValueError):
                modulation_routes(modulation_fixture()[:size])
        for offset, value in [(40, 33), (64, 0xFFFFFFFF), (64, 0x7FFFFFFF),
                              (80, 0x8F050010)]:
            with self.subTest(offset=offset, value=value), self.assertRaises(ValueError):
                data = modulation_fixture()
                struct.pack_into('>I', data, offset, value)
                modulation_routes(data)


if __name__ == '__main__':
    unittest.main()
