import copy
import struct
import unittest

from tools.asset_pipeline.vlt import hash64
from tools.owned_game.impact_intensity import CLASS, REFERENCE, intensity_tables


def fixture():
    # Independent schema-order fixture: high, upper scale, medium, minimum.
    values = [('C8DED1BC20B9D6A5', .375), ('B8870D2001033E0F', .65),
              ('7D8DEDD338D45482', .05), ('D660AC459139BDF4', .0025)]
    mapping = {'class': CLASS, 'key': 'face_head', 'fields': {
        'Hash_' + field: {'type': 'EA::Reflection::Float',
                         'data': struct.pack('>f', number).hex()}
        for field, number in values}}
    material = {'class': 'aud_material', 'key': 'head', 'parent': 'other',
                'fields': {REFERENCE: {'type': 'Attrib::RefSpec', 'data':
                    struct.pack('>QQQ', hash64(CLASS), hash64('face_head'), 0).hex()}}}
    return [mapping, material]


class ImpactIntensityTests(unittest.TestCase):
    def test_resolves_reference_instead_of_material_or_parent_name(self):
        result = intensity_tables(fixture())
        self.assertEqual(result['materials'], {'head': 'face_head'})
        table = result['tables']['face_head']
        for name, number in [('minimum', .0025), ('medium', .05),
                             ('hard', .375), ('maximum', .65)]:
            self.assertAlmostEqual(table[name], number)

    def test_does_not_reorder_authored_boundaries(self):
        rows = fixture()
        rows[0]['fields']['Hash_7D8DEDD338D45482']['data'] = struct.pack('>f', 2).hex()
        table = intensity_tables(rows)['tables']['face_head']
        self.assertEqual(table['medium'], 2)
        self.assertEqual(table['hard'], .375)

    def test_rejects_wrong_class_unresolved_and_truncated_references(self):
        for raw in [struct.pack('>QQQ', hash64('another_class'), hash64('face_head'), 0),
                    struct.pack('>QQQ', hash64(CLASS), hash64('missing'), 0), b'\0' * 8]:
            with self.subTest(raw=raw.hex()), self.assertRaises(ValueError):
                rows = fixture()
                rows[1]['fields'][REFERENCE]['data'] = raw.hex()
                intensity_tables(rows)

    def test_rejects_invalid_scalars_and_duplicate_identities(self):
        for number in [-1, float('nan'), float('inf')]:
            with self.subTest(number=number), self.assertRaises(ValueError):
                rows = fixture()
                rows[0]['fields']['Hash_D660AC459139BDF4']['data'] = struct.pack('>f', number).hex()
                intensity_tables(rows)
        for index in [0, 1]:
            rows = fixture()
            rows.append(copy.deepcopy(rows[index]))
            with self.subTest(index=index), self.assertRaisesRegex(ValueError, 'Duplicate'):
                intensity_tables(rows)


if __name__ == '__main__':
    unittest.main()
