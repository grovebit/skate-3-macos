import struct
import unittest

from tools.owned_game.audio_surface import SURFACE_MAP, surface_classes, surface_entries


def fixture():
    items = []
    for index in range(95):
        entry = bytearray(72)
        struct.pack_into('>I', entry, 8, 1)  # All wheel modes equal; classes differ.
        struct.pack_into('>i', entry, 0x1c, index % 3)
        items.append(entry.hex())
    return [{'class': 'aud_general', 'key': 'mapping', 'fields': {
        SURFACE_MAP: {'array': {'element_size': 72, 'items': items}}}}]


class AudioSurfaceTests(unittest.TestCase):
    def test_preserves_classes_independently_of_wheel_mode_including_fallback(self):
        self.assertEqual(surface_classes(fixture()), [i % 3 for i in range(95)])

    def test_rejects_missing_duplicate_and_truncated_mappings(self):
        rows = fixture()
        for bad in ([], rows * 2):
            with self.assertRaises(ValueError):
                surface_entries(bad)
        array = rows[0]['fields'][SURFACE_MAP]['array']
        array['items'][12] = '00' * 71
        with self.assertRaises(ValueError):
            surface_entries(rows)

    def test_rejects_unsupported_signed_class(self):
        for invalid in (-1, 3):
            rows = fixture()
            items = rows[0]['fields'][SURFACE_MAP]['array']['items']
            entry = bytearray.fromhex(items[94])
            struct.pack_into('>i', entry, 0x1c, invalid)
            items[94] = entry.hex()
            with self.assertRaises(ValueError):
                surface_classes(rows)
