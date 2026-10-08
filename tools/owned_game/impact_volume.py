"""Authored integer levels consumed by base-disc 82484A98..82484C34.

Ordinary volume RefSpecs are selected by 82484C38. Special-mode overrides and
runtime voice gain remain separate, unported dependencies.
"""
import math
import struct

from tools.owned_game.impact_intensity import material_tables

CLASS = 'aud_volumemapping'
REFERENCE = 'Hash_82B1451A90152514'
HARD_LAYER_SCALE = 'Hash_1A5F7E8CCABBB0A2'  # layout +28; 82484D90
# Names identify intensity category and counterpart class (82485750), not
# physical surface hardness. Body materials >= 0x61 use counterpart class 0.
FIELDS = {
    'medium_0_min': 'Hash_036F313CEBC664FD',  # +00
    'medium_0_max': 'Hash_8001982DA2E91D6A',  # +04
    'soft_0_min': 'Hash_E24D9CB4000A53AC',    # +08
    'soft_0_max': 'Hash_1A83AD0330976744',    # +0C
    'medium_1_min': 'Hash_24B725E05CEB027E',  # +10
    'medium_1_max': 'Hash_CBBB19E302CE1A17',  # +14
    'soft_1_min': 'Hash_75DC915E876A9DC9',    # +18
    'soft_1_max': 'Hash_711F1F54903E76C9',    # +1C
    'hard_min': 'Hash_C8DED1BC20B9D6A5',     # +20
    'hard_max': 'Hash_B8870D2001033E0F',     # +24
    'medium_2_min': 'Hash_41F2E2456A97752A',  # +2C
    'medium_2_max': 'Hash_2638FF12C8FBBCCB',  # +30
    'soft_2_min': 'Hash_166BAB2FD5B60560',    # +34
    'soft_2_max': 'Hash_0D6EF57A39AF0C93',    # +38
}


def volume_tables(rows):
    result = material_tables(rows, CLASS, REFERENCE, FIELDS, 'EA::Reflection::Int32', '>i', allow_null=True)
    result['fields']['hard_layer_scale'] = HARD_LAYER_SCALE
    for row in rows:
        if row['class'] != CLASS:
            continue
        field = row['fields'][HARD_LAYER_SCALE]
        raw = bytes.fromhex(field['data'])
        if field['type'] != 'EA::Reflection::Float' or len(raw) != 4:
            raise ValueError(f"{row['key']}: unsupported hard-layer scale")
        scale, = struct.unpack('>f', raw)
        if not math.isfinite(scale) or scale < 0:
            raise ValueError(f"{row['key']}: invalid hard-layer scale")
        result['tables'][row['key']]['hard_layer_scale'] = scale
    return result
