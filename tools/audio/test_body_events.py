import copy
import struct
import unittest

from tools.owned_game.body_events import body_event_routes
from tools.audio.prepare_impact_audio import select_event


def fixture():
    # Independent field order follows native soft/medium/hard reads.
    fields = ['BFABF634D2B1E45A', '9ABFC64574AB2F9F', 'BCD5E888294F7B15',
              'EF9BD81F9CFF725F', 'A3ADCA7B19287B5D', 'C676C87F862C0490',
              '9203DF6FD029B377']
    rows = [{'class': 'aud_material', 'key': name, 'fields': {
        'Hash_' + key: {'type': 'Skate_Collisions',
                       'data': struct.pack('>I', i).hex()}
        for i, key in enumerate(fields)}} for name in ['head', 'torso', 'arm', 'leg']]
    events = {'Skate_Collisions': [[[[i + 1]]] for i in range(7)]}
    return rows, events


class BodyEventTests(unittest.TestCase):
    def test_categories_and_counterpart_classes_select_distinct_events(self):
        rows, events = fixture()
        result = body_event_routes(rows, events, select_event)
        self.assertEqual(set(result), {'head', 'torso', 'arm', 'leg'})
        for categories in result.values():
            self.assertEqual([[r['event'] for r in c] for c in categories],
                             [[0, 1, 2], [3, 4, 5], [6, 6, 6]])
        self.assertEqual(result['head'][0][0]['source_layers'], [['Skate_Collisions/0001.wav']])

    def test_rejects_missing_or_duplicate_primary_materials(self):
        rows, events = fixture()
        for bad in [rows[:-1], rows + [copy.deepcopy(rows[0])]]:
            with self.assertRaises(ValueError):
                body_event_routes(bad, events, select_event)

    def test_rejects_wrong_bank_truncated_and_out_of_range_events(self):
        for value in [{'type': 'Skate_Metal', 'data': '00000000'},
                      {'type': 'Skate_Collisions', 'data': '0000'},
                      {'type': 'Skate_Collisions', 'data': 'FFFFFFFF'}]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                rows, events = fixture()
                rows[0]['fields']['Hash_BFABF634D2B1E45A'] = value
                body_event_routes(rows, events, select_event)

    def test_selected_events_retain_simultaneous_layers(self):
        rows, events = fixture()
        events['Skate_Collisions'][6] = [[[41], [42]]]
        result = body_event_routes(rows, events, select_event)
        self.assertEqual(result['head'][2][0]['source_layers'],
                         [['Skate_Collisions/0041.wav', 'Skate_Collisions/0042.wav']])


if __name__ == '__main__':
    unittest.main()
