"""Ordinary primary-body voice controls from base-disc 82484410/824BFF88.

Keep authored integers. The voice consumer combines them with live packed
controller values; these are not standalone normalized host gains or pitches.
"""
import struct

from tools.owned_game.body_events import PARTS

BASE_LEVEL = 'Hash_875BA75341DC8391'  # aud_material +34
PITCH = 'Hash_C090F2C1F048F17B'       # aud_material +44
KIND = 'Hash_D5EF686287A57AFE'        # aud_material +48
# 824BFFCC jump table: material class 0..9 -> volume controller slot.
VOLUME_SLOTS = (13, 14, 15, 16, 17, 18, 12, 19, 20, 21)


def body_controls(rows):
    result = {}
    for row in rows:
        if row['class'] != 'aud_material' or row['key'] not in PARTS:
            continue
        name = row['key']
        if name in result:
            raise ValueError(f'Duplicate body material: {name}')

        def integer(field, expected_type):
            value = row['fields'][field]
            raw = bytes.fromhex(value['data'])
            if value['type'] != expected_type or len(raw) != 4:
                raise ValueError(f'{name}: invalid voice control {field}')
            return struct.unpack('>i', raw)[0]

        authored_level = integer(BASE_LEVEL, 'EA::Reflection::Int32')
        pitch = integer(PITCH, 'EA::Reflection::Int32')
        kind = integer(KIND, 'Sk8::Audio::eMaterialNicotineType')
        # 824BFFAC uses unsigned comparison; out-of-range classes select slot20.
        slot = VOLUME_SLOTS[kind] if 0 <= kind < len(VOLUME_SLOTS) else 20
        result[name] = {
            'authored_base_level': authored_level,
            'base_level': min(32767, max(0, authored_level)),
            'pitch': pitch,
            'material_class': kind,
            'volume_controller_slot': slot,
        }
    if set(result) != set(PARTS):
        raise ValueError('Missing primary body voice controls')
    return {'fields': {'base_level': BASE_LEVEL, 'pitch': PITCH, 'material_class': KIND},
            'materials': result}
