import struct
import unittest

from tools.owned_game.splc import event_samples, event_layers
from tools.audio.prepare_impact_audio import select_material, SURFACES
from tools.owned_game.contact_audio import contact_routes, FIELDS
from tools.audio.prepare_impact_audio import select_event
from tools.audio import test_prepare_skating_audio as fixtures


def control_bank():
    # Event/leaf indices intentionally differ from embedded sample indices.
    base = fixtures.SplcTests.bank()
    header = bytearray(base[:60])
    leaves = bytearray(72)
    struct.pack_into('>IHH', leaves, 0, 0, 0, 2)
    struct.pack_into('>IHH', leaves, 36, 0, 1, 1)
    group = struct.pack('>I32H4B', 0x10000, 0, 1, *([0xffff] * 30), 2, 2, 0, 0)
    layers = bytearray()
    for samples in [[1], [0, 1], [0]]:
        layers += b'\0\0\0\0\0\1\0\0' + bytes([len(samples), 1, 0, 0])
        for sample in samples:
            record = bytearray(72)
            struct.pack_into('>H', record, 0, sample)
            layers += record
    control = leaves + group + layers
    struct.pack_into('>4I', header, 8, len(control), 2, 1, 0)
    return header + control + base[60:]


class ImpactAudioTests(unittest.TestCase):
    def test_contact_tables_preserve_empty_redirected_slots_and_authored_gains(self):
        scalar = lambda value: struct.pack('>f', value).hex()
        contacts = {
            'Hash_2A70BB8A382574E4': {'data': scalar(.31)},
            'Hash_6D3D91A9BA7ADCDC': {'data': scalar(.5)},
            # Bool occupies the first byte of its padded storage, not a u32.
            'Hash_642CF9BFEC6BE988': {'data': '01000000'},
        }
        for mode, (event, gain) in enumerate(FIELDS):
            contacts['Hash_' + event] = {'type': 'Skate_Collisions', 'array': {
                'items': ['00000001' if mode and i in (2, 5, 8) else '00000000'
                          for i in range(13)]}}
            contacts['Hash_' + gain] = {'array': {'items': [scalar(.84)] * 15}}
        rows = [{'class': 'aud_contacts', 'key': 'default', 'fields': contacts},
                {'class': 'aud_general', 'key': 'mapping', 'fields': {
                    'Hash_4CA607558B1CF440': {'array': {'element_size': 72,
                        'items': ['000000000000000000000001' + '00' * 60] * 95}}}}]
        events = {'Skate_Collisions': [[[[22]]], [[], []]]}
        tables = contact_routes(rows, events, select_event)
        self.assertIsNone(tables['modes'][1][2])
        self.assertEqual(tables['modes'][0][2]['source_layers'], [['Skate_Collisions/0022.wav']])
        self.assertAlmostEqual(tables['volumes'][3][0], .84)
        self.assertTrue(all(tables['surface_modes']))
        contacts['Hash_' + FIELDS[0][0]]['array']['items'][0] = '0000FFFF'
        with self.assertRaisesRegex(ValueError, 'outside'):
            contact_routes(rows, events, select_event)

    def test_resolves_groups_layers_and_choices_to_one_based_stream_ids(self):
        self.assertEqual(event_samples(control_bank()), [[2, 1], [1], [2, 1]])

    def test_preserves_layer_boundaries_and_group_variants(self):
        self.assertEqual(event_layers(control_bank()),
                         [[[[2], [1, 2]]], [[[1]]], [[[2], [1, 2]], [[1]]]])

    def test_rejects_unknown_layout_and_invalid_sample_or_group_references(self):
        for offset, value in [(60, 1), (60 + 4, 1), (60 + 72 + 4, 2),
                              (60 + 72 + 72 + 12, 2), (60 + 72 + 72 + 8, 0)]:
            data = control_bank()
            if offset == 60 + 72 + 72 + 8:
                data[offset] = value
            else:
                struct.pack_into('>H', data, offset, value)
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                event_samples(data)
        with self.assertRaises(ValueError):
            event_samples(control_bank()[:-70])

    def test_material_selection_uses_event_references_not_event_as_sample_id(self):
        row = {'key': 'wood_1', 'fields': {
            'Hash_BFABF634D2B1E45A': {'type': 'Skate_Collisions', 'data': '00000002'}}}
        events = {'Skate_Collisions': [[[[1]]], [[[1]]], [[[22]], [[24]], [[23]]]]}
        route = select_material(row, events)
        self.assertEqual(route['event'], 2)
        self.assertEqual(route['source_layers'], [['Skate_Collisions/0022.wav'],
                                                  ['Skate_Collisions/0024.wav'],
                                                  ['Skate_Collisions/0023.wav']])
        self.assertEqual(route['clips'][0], 'Skate_Collisions/events/0002-0.wav')
        row['fields']['Hash_BFABF634D2B1E45A']['data'] = '00000000'
        self.assertIsNone(select_material(row, events))
        row['fields']['Hash_BFABF634D2B1E45A']['data'] = '00000003'
        with self.assertRaises(ValueError):
            select_material(row, events)

    def test_selection_keeps_layers_together_without_mixing_group_alternatives(self):
        row = {'key': 'torso', 'fields': {
            'Hash_BFABF634D2B1E45A': {'type': 'Skate_Collisions', 'data': '00000001'}}}
        events = {'Skate_Collisions': [[], [[[10, 11, 12], [20]], [[30], [40, 41]]]]}
        route = select_material(row, events)
        self.assertEqual(route['source_layers'], [
            ['Skate_Collisions/0010.wav', 'Skate_Collisions/0020.wav'],
            ['Skate_Collisions/0030.wav', 'Skate_Collisions/0040.wav'],
            ['Skate_Collisions/0011.wav', 'Skate_Collisions/0020.wav'],
        ])

    def test_surface_enum_keeps_collision_tag_indices(self):
        self.assertEqual(SURFACES[3], 'concrete_polished')
        self.assertEqual(SURFACES[6], 'wood_ramp')
        self.assertEqual(SURFACES[11], 'metal_solid_round_1')
        self.assertEqual(SURFACES[41], 'wood_1')


if __name__ == '__main__':
    unittest.main()
