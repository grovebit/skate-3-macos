import copy
import struct
import unittest

from tools.owned_game.collision_materials import (
    AUDIO_MATERIALS, BANKS, BANDS, COOLDOWN, BOARD_COOLDOWN, EVENT_FIELDS, body_settings, collision_materials)


def fixture():
    rows = {}
    for index, (name, bank) in enumerate(AUDIO_MATERIALS):
        if name is None or name in rows:
            continue
        fields = {'Hash_' + key: {'type': BANKS[bank], 'data': struct.pack('>I', 10 * index + i).hex()}
                  for i, key in enumerate(EVENT_FIELDS[bank])}
        fields['Hash_875BA75341DC8391'] = {'type': 'EA::Reflection::Int32', 'data': struct.pack('>i', 40000).hex()}
        fields['Hash_C090F2C1F048F17B'] = {'type': 'EA::Reflection::Int32', 'data': struct.pack('>i', 3796).hex()}
        fields['Hash_D5EF686287A57AFE'] = {'type': 'Sk8::Audio::eMaterialNicotineType',
                                           'data': struct.pack('>i', bank).hex()}
        rows[name] = {'class': 'aud_material', 'key': name, 'fields': fields}
    settings = {COOLDOWN: {'type': 'EA::Reflection::Int32', 'data': '0000000F'}}
    settings[BOARD_COOLDOWN] = {'type': 'EA::Reflection::Int32', 'data': '00000006'}
    settings.update({field: {'type': 'EA::Reflection::Float', 'data': struct.pack('>f', i / 4).hex()}
                     for i, field in enumerate(BANDS)})
    return [*rows.values(), {'class': 'aud_collisions', 'key': 'default', 'fields': settings}]


class CollisionMaterialTests(unittest.TestCase):
    def test_ids_follow_the_native_table_including_shared_and_empty_rows(self):
        result = collision_materials(fixture())
        self.assertEqual(len(result), 0x8F)
        self.assertIsNone(result[0x5E])
        # Snow and ice tags reuse grass, so they keep grass's first-ID events.
        self.assertEqual(result[0x45], result[0x09])
        self.assertEqual(result[0x08]['name'], 'default_metal')
        self.assertEqual(result[0x08]['bank'], 1)
        self.assertEqual(result[0x62]['events'], {'soft': [980, 981, 982], 'medium': [983, 984, 985], 'hard': 986})
        self.assertEqual(result[0x66]['bank'], 2)

    def test_controls_keep_authored_values_and_clamp_only_the_base_level(self):
        torso = collision_materials(fixture())[0x62]
        self.assertEqual((torso['authored_base_level'], torso['base_level'], torso['pitch']), (40000, 32767, 3796))

    def test_bank_types_must_match_the_material_bank(self):
        rows = fixture()
        metal = next(r for r in rows if r['key'] == 'metal_sheet')
        metal['fields']['Hash_' + EVENT_FIELDS[1][0]]['type'] = 'Skate_Collisions'
        with self.assertRaises(ValueError):
            collision_materials(rows)

    def test_missing_or_duplicate_rows_are_rejected(self):
        rows = fixture()
        with self.assertRaises(ValueError):
            collision_materials([r for r in rows if r['key'] != 'facehit'])
        with self.assertRaises(ValueError):
            collision_materials(rows + [copy.deepcopy(rows[0])])

    def test_body_settings_preserve_cooldown_and_band_order(self):
        settings = body_settings(fixture())
        self.assertEqual(settings['cooldown'], 15)
        self.assertEqual(settings['board_cooldown'], 6)
        self.assertEqual(settings['bands'], [i / 4 for i in range(10)])
        with self.assertRaises(ValueError):
            body_settings([r for r in fixture() if r['class'] != 'aud_collisions'])

    def test_grind_bands_require_the_authored_rail_row(self):
        rows = fixture()
        self.assertNotIn('grind_bands', body_settings(rows))
        rail = {'class': 'aud_rails', 'key': 'default', 'fields': {
            field: {'type': 'EA::Reflection::Float', 'data': struct.pack('>f', value).hex()}
            for field, value in [('Hash_086B66C3D4FFEE8F', 0.25), ('Hash_B2ACAFDBCD963C93', 0.5)]}}
        self.assertEqual(body_settings(rows + [rail])['grind_bands'], [0.25, 0.5])
        with self.assertRaises(ValueError):
            body_settings(rows + [rail, rail])


if __name__ == '__main__':
    unittest.main()
