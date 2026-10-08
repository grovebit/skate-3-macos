import copy
import struct
import unittest

from tools.asset_pipeline.vlt import hash64
from tools.owned_game.impact_volume import CLASS, REFERENCE, volume_tables


def fixture():
    # Independent schema order; distinct values detect swapped category/class pairs.
    hashes = ['036F313CEBC664FD', '8001982DA2E91D6A', 'E24D9CB4000A53AC',
              '1A83AD0330976744', '24B725E05CEB027E', 'CBBB19E302CE1A17',
              '75DC915E876A9DC9', '711F1F54903E76C9', 'C8DED1BC20B9D6A5',
              'B8870D2001033E0F', '41F2E2456A97752A', '2638FF12C8FBBCCB',
              '166BAB2FD5B60560', '0D6EF57A39AF0C93']
    mapping = {'class': CLASS, 'key': 'face_head', 'fields': {
        'Hash_' + field: {'type': 'EA::Reflection::Int32',
                         'data': struct.pack('>i', i * 1001).hex()}
        for i, field in enumerate(hashes)}}
    mapping['fields']['Hash_1A5F7E8CCABBB0A2'] = {
        'type': 'EA::Reflection::Float', 'data': struct.pack('>f', .625).hex()}
    material = {'class': 'aud_material', 'key': 'head', 'parent': 'other',
                'fields': {REFERENCE: {'type': 'Attrib::RefSpec', 'data':
                    struct.pack('>QQQ', hash64(CLASS), hash64('face_head'), 0).hex()}}}
    return [mapping, material]


class ImpactVolumeTests(unittest.TestCase):
    def test_resolves_reference_and_preserves_all_integer_pairs(self):
        result = volume_tables(fixture())
        self.assertEqual(result['materials'], {'head': 'face_head'})
        table = result['tables']['face_head']
        self.assertEqual(table.pop('hard_layer_scale'), .625)
        self.assertEqual(table, dict(zip([
            'medium_0_min', 'medium_0_max', 'soft_0_min', 'soft_0_max',
            'medium_1_min', 'medium_1_max', 'soft_1_min', 'soft_1_max',
            'hard_min', 'hard_max', 'medium_2_min', 'medium_2_max',
            'soft_2_min', 'soft_2_max'], [i * 1001 for i in range(14)])))
        self.assertTrue(all(type(value) is int for value in table.values()))

    def test_preserves_descending_levels_without_normalization(self):
        rows = fixture()
        rows[0]['fields']['Hash_036F313CEBC664FD']['data'] = struct.pack('>i', 32767).hex()
        table = volume_tables(rows)['tables']['face_head']
        self.assertEqual((table['medium_0_min'], table['medium_0_max']), (32767, 1001))

    def test_retains_null_profile_without_inventing_a_default(self):
        rows = fixture()
        rows[1]['fields'][REFERENCE]['data'] = struct.pack('>QQQ', hash64(CLASS), 0, 0).hex()
        self.assertIsNone(volume_tables(rows)['materials']['head'])

    def test_rejects_wrong_types_truncation_and_negative_levels(self):
        for field in [{'type': 'EA::Reflection::Float', 'data': '3F800000'},
                      {'type': 'EA::Reflection::Int32', 'data': '0000'},
                      {'type': 'EA::Reflection::Int32', 'data': 'FFFFFFFF'}]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                rows = fixture()
                rows[0]['fields']['Hash_036F313CEBC664FD'] = field
                volume_tables(rows)

    def test_rejects_invalid_layer_scales_and_preserves_values_above_one(self):
        for field in [{'type': 'EA::Reflection::Int32', 'data': '00000001'},
                      {'type': 'EA::Reflection::Float', 'data': '0000'},
                      *[{'type': 'EA::Reflection::Float', 'data': struct.pack('>f', v).hex()}
                        for v in [-1, float('nan'), float('inf')]]]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                rows = fixture()
                rows[0]['fields']['Hash_1A5F7E8CCABBB0A2'] = field
                volume_tables(rows)
        rows = fixture()
        rows[0]['fields']['Hash_1A5F7E8CCABBB0A2']['data'] = struct.pack('>f', 1.5).hex()
        self.assertEqual(volume_tables(rows)['tables']['face_head']['hard_layer_scale'], 1.5)

    def test_rejects_wrong_class_missing_reference_and_duplicate_identity(self):
        for cls, key in [('aud_intensitymapping', 'face_head'), (CLASS, 'missing')]:
            with self.subTest(cls=cls, key=key), self.assertRaises(ValueError):
                rows = fixture()
                rows[1]['fields'][REFERENCE]['data'] = struct.pack('>QQQ', hash64(cls), hash64(key), 0).hex()
                volume_tables(rows)
        for i in [0, 1]:
            rows = fixture()
            rows.append(copy.deepcopy(rows[i]))
            with self.assertRaisesRegex(ValueError, 'Duplicate'):
                volume_tables(rows)


if __name__ == '__main__':
    unittest.main()
