"""Collision materials by original ID: base-disc table 82FD1930 and 82484410.

Each 16-byte entry stores the bank in word +00 and the aud_material key in
+08. Names below were resolved by matching those keys with the owned database
(default.xex SHA-256 1db39496...cf7f); several IDs share one row, e.g. the
snow and ice tags use grass. They are audio material IDs, not the collision-tag
names in prepare_impact_audio.SURFACES. Event fields follow 82484638..824849C4.
"""
import math
import struct

# (aud_material key, bank) for IDs 0x00..0x8E. Bank -1 marks the empty entry.
AUDIO_MATERIALS = (
    ('asphalt_smooth', 0), ('asphalt_rough', 0), ('concrete_polished', 0),
    ('concrete_rough', 0), ('concrete_aggregate', 0), ('wood_ramp', 0), ('plywood', 0),
    ('dirt', 0), ('default_metal', 1), ('grass', 0), ('metal_solid_round_1', 1),
    ('metal_solid_round_1_up', 1), ('metal_solid_round_2', 1), ('metal_solid_square_1', 1),
    ('metal_solid_square_2', 1), ('metal_hollow_round_1', 1), ('metal_hollow_round_1_dead', 1),
    ('metal_hollow_round_1_dn', 1), ('metal_hollow_round_2', 1), ('metal_hollow_round_2_dead', 1),
    ('metal_hollow_round_2_dn', 1), ('metal_hollow_round_3', 1), ('metal_hollow_round_4', 1),
    ('metal_hollow_square_1', 1), ('metal_hollow_square_2', 1), ('metal_hollow_square_3', 1),
    ('metal_hollow_square_3_dead', 1), ('metal_hollow_square_4', 1), ('metal_hollow_1', 1),
    ('metal_hollow_2', 1), ('metal_sheet', 1), ('metal_complex_1', 1), ('metal_complex_2', 1),
    ('metal_complex_3', 1), ('metal_complex_4', 1), ('metal_complex_5', 1), ('metal_complex_6', 1),
    ('metal_complex_7', 1), ('metal_complex_8', 1), ('metal_complex_debris', 1), ('wood_1', 0),
    ('wood_1_up', 0), ('wood_2', 0), ('wood_3', 0), ('wood_3_up', 0), ('wood_4', 0),
    ('plastic_1', 0), ('plastic_2', 0), ('plastic_3', 0), ('plastic_4', 0),
    ('glass_thick_large', 0), ('glass_thin_small', 0), ('concrete_curb', 0),
    ('concrete_bench', 0), ('leaves', 0), ('bush', 0), ('pottery', 0), ('paper', 0),
    ('cardboard', 0), ('garbage_bag', 0), ('garbage_spill', 0), ('bottle', 0),
    ('tile_ceramic', 0), ('marble_or_slate', 0), ('brick_smooth', 0), ('brick_course', 0),
    ('manhole_metal', 1), ('metal_grate_sewer', 1), ('metal_grate_planter', 1), ('grass', 0),
    ('grass', 0), ('grass', 0), ('antennas', 1), ('chandelier', 0), ('plexiglass_small', 0),
    ('plexiglass_large', 0), ('potted_plant', 0), ('crumpled_paper', 0), ('cloth', 0),
    ('pop_can', 0), ('paper_cup', 0), ('wire_cable', 1), ('volleyball', 0), ('oil_drum', 1),
    ('dmo_rail', 1), ('fruit', 0), ('plastic_bottle', 0), ('drum_pylon', 0),
    ('metal_rail_4', 1), ('wood_5', 0), ('metal_ramp', 1), ('complex_plastic_1', 0),
    ('water', 0), ('inflated_balloon', 0), (None, -1), ('board', 0), ('truck', 0),
    ('head', 0), ('torso', 0), ('leg', 0), ('arm', 0), ('foot', 0), ('hom_head', 2),
    ('hom_torso', 2), ('hom_leg', 2), ('hom_arm', 2), ('hom_foot', 2), ('skin', 0),
    ('denim', 0), ('cotton', 0), ('bonecrack', 0), ('bonesnap', 0), ('facehit', 0),
    ('boardtumble', 0), ('metal_hollow_square_3_dead_rattle', 1), ('metal_hollow_1_rattle', 1),
    ('metal_hollow_square_3_rattle', 1), ('metal_hollow_2_rattle', 1), ('dmorail_rattle', 1),
    ('antennas_rattle', 1), ('metal_complex_5_rattle', 1), ('metal_sheet_rattle', 1),
    ('metal_complex_1_rattle', 1), ('wood_1_rattle', 0), ('wood_2_rattle', 0),
    ('wood_3_rattle', 0), ('wood_4_rattle', 0), ('plastic_1_rattle', 0), ('plastic_2_rattle', 0),
    ('plastic_3_rattle', 0), ('plastic_4_rattle', 0), ('cardboard_rattle', 0),
    ('pottery_rattle', 0), ('pottery_2_rattle', 0), ('terrain_concrete_small', 0),
    ('terrain_concrete_medium', 0), ('terrain_concrete_large', 0), ('terrain_wood_small', 0),
    ('terrain_wood_medium', 0), ('terrain_wood_large', 0), ('terrain_generic_small', 0),
    ('terrain_generic_medium', 0), ('terrain_generic_large', 0),
)
BANKS = ('Skate_Collisions', 'Skate_Metal', 'HOM_Set_1')
# Soft classes 0/1/2, medium classes 0/1/2, then hard. Banks 0 and 1 read
# fixed row offsets; bank 2 looks these keys up (82470500 supplies soft 0).
EVENT_FIELDS = (
    ('BFABF634D2B1E45A', '9ABFC64574AB2F9F', 'BCD5E888294F7B15',
     'EF9BD81F9CFF725F', 'A3ADCA7B19287B5D', 'C676C87F862C0490', '9203DF6FD029B377'),
    ('66A95889604DED36', '595537EBA0196BE7', '79DD0E6659793D0E',
     'B722B88FE44B046E', '1411108A7E9CC74A', '50796F92F3DE449B', 'F54277A83E0170FD'),
    ('137C683BFB506ECA', '2A2830137430BB02', '79BDE00B04DF51B9',
     '5432B35224E4B1C1', '3FCBE0407F833721', '53311C6761F135A1', '3EA2579C2F3BB23B'),
)
BASE_LEVEL = 'Hash_875BA75341DC8391'  # +34, clamped by 824844D4..824844F4
PITCH = 'Hash_C090F2C1F048F17B'       # +44
KIND = 'Hash_D5EF686287A57AFE'        # +48
# aud_collisions/default: the body cooldown, then 824A9CD8's Contacts +128..+14C.
COOLDOWN = 'Hash_6DD85F43C1B6E6AA'
BOARD_COOLDOWN = 'Hash_27D3C5DC3282B59D'
BANDS = ('Hash_AFA4B5090F1BCF36', 'Hash_00960FEDB3EFEE9C', 'Hash_D12003A60E987B9D',
         'Hash_FA2AA5A0C0481D00', 'Hash_3695327CFB5E1AC3', 'Hash_35FEE8A95523D812',
         'Hash_076E9081CA1759E9', 'Hash_DF539915EB7E883E', 'Hash_8E3025BAA686F721',
         'Hash_504D3B73505972D4')
# The layout's only field, at offset 0. Publisher 8249F794..8249F7CC passes its
# x (+10) and y (+30) arrays to 8246FD40; the header words +00..+0C are unread.
SPEED_GRAPH = 'Hash_8B164823E008749C'


def _scalar(row, field, expected_type, fmt):
    value = row['fields'].get(field)
    if value is None or value['type'] != expected_type:
        raise ValueError(f"{row['key']}: missing or mistyped {field}")
    raw = bytes.fromhex(value['data'])
    if len(raw) != 4:
        raise ValueError(f"{row['key']}: invalid {field}")
    return struct.unpack(fmt, raw)[0]


def _speed_graph(row):
    value = row['fields'].get(SPEED_GRAPH)
    if value is None or value['type'] != 'Sk8::PointNegGraphData8':
        raise ValueError(f"{row['key']}: missing or mistyped {SPEED_GRAPH}")
    raw = bytes.fromhex(value['data'])
    if len(raw) != 80:
        raise ValueError(f"{row['key']}: invalid {SPEED_GRAPH}")
    words = struct.unpack('>20f', raw)
    if not all(map(math.isfinite, words)):
        raise ValueError(f"{row['key']}: nonfinite {SPEED_GRAPH}")
    return {'x': list(words[4:12]), 'y': list(words[12:20])}


def collision_materials(rows):
    """Return one entry per material ID, or None for the empty table entry."""
    materials = {}
    for row in rows:
        if row['class'] == 'aud_material':
            if row['key'] in materials:
                raise ValueError(f"Duplicate audio material: {row['key']}")
            materials[row['key']] = row
    result = []
    for name, bank in AUDIO_MATERIALS:
        if name is None:
            result.append(None)
            continue
        row = materials.get(name)
        if row is None:
            raise ValueError(f'Missing audio material {name}')
        # Event zero is a valid bank event, not an absent reference.
        events = [_scalar(row, 'Hash_' + field, BANKS[bank], '>I') for field in EVENT_FIELDS[bank]]
        authored = _scalar(row, BASE_LEVEL, 'EA::Reflection::Int32', '>i')
        result.append({
            'name': name, 'bank': bank,
            'events': {'soft': events[0:3], 'medium': events[3:6], 'hard': events[6]},
            'authored_base_level': authored, 'base_level': min(32767, max(0, authored)),
            'pitch': _scalar(row, PITCH, 'EA::Reflection::Int32', '>i'),
            'material_class': _scalar(row, KIND, 'Sk8::Audio::eMaterialNicotineType', '>i'),
        })
    return result


def body_settings(rows):
    defaults = [r for r in rows if r['class'] == 'aud_collisions' and r['key'] == 'default']
    if len(defaults) != 1:
        raise ValueError('Expected one aud_collisions/default row')
    row = defaults[0]
    settings = {'cooldown': _scalar(row, COOLDOWN, 'EA::Reflection::Int32', '>i'),
            'board_cooldown': _scalar(row, BOARD_COOLDOWN, 'EA::Reflection::Int32', '>i'),
            'bands': [_scalar(row, field, 'EA::Reflection::Float', '>f') for field in BANDS],
            'speed_graph': _speed_graph(row)}
    rails = [r for r in rows if r['class'] == 'aud_rails' and r['key'] == 'default']
    if len(rails) > 1:
        raise ValueError('Expected at most one aud_rails/default row')
    if rails:
        settings['grind_bands'] = [_scalar(rails[0], field, 'EA::Reflection::Float', '>f')
            for field in ['Hash_086B66C3D4FFEE8F', 'Hash_B2ACAFDBCD963C93']]
    return settings
