"""Verified enabled format-0 MixMap conversion; no live-input/default inference.

Base-disc 82928658..82928724 and 829288CC..829289C8 add signed levels,
clamp, then use a 602-word table at 82FBB430. The packed output write keeps
the low 16 bits. Other formats, disabled routes and special writes are outside
this helper's scope. The caller supplies the table from its owned executable.
"""
import struct


def read_volume_table(data):
    if len(data) != 602 * 4:
        raise ValueError('MixMap volume table must contain exactly 602 big-endian words')
    return struct.unpack('>602i', data)


def format0_volume(record_level, adjustment, table, modulation_level=None):
    """Convert explicit evaluated operands, preserving PPC signed-word overflow.

    modulation_level=None selects the path without a modulation node. A node's
    numeric zero selects the modulated path; both paths share this conversion.
    This does not evaluate the expressions that produce either level.
    """
    for value in (record_level, modulation_level):
        if value is not None and not -(1 << 31) <= value < (1 << 31):
            raise ValueError('Evaluated levels must be signed 32-bit integers')
    if not -(1 << 15) <= adjustment < (1 << 15):
        raise ValueError('Write adjustment must be a signed 16-bit integer')
    if len(table) != 602 or any(not -(1 << 31) <= v < (1 << 31) for v in table):
        raise ValueError('MixMap volume table must contain 602 signed 32-bit words')
    total = record_level + adjustment
    if modulation_level is not None:
        total += modulation_level
    summed = ((total + (1 << 31)) & 0xFFFFFFFF) - (1 << 31)
    clamped = min(0, max(-10000, summed))
    shift, remainder = divmod(-clamped, 602)
    index = 601 - remainder
    converted = 0 if shift > 15 else table[index] >> shift
    return {'summed_level': summed, 'clamped_level': clamped,
            'table_index': None if shift > 15 else index, 'shift': shift,
            'packed_halfword': converted & 0xFFFF}
