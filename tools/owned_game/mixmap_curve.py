"""Original MixMap 15-bit curve evaluator (82923F18..82924100).

The caller supplies the owned 513-word table at base-disc 82FBC598. Integer
inputs are restricted to 0..32767; the 82923EA8 float wrapper accepts normalized
binary32 inputs in 0..1. Neither reproduces spatial/phase calculations. The
integer interpolation weight is not a generic lerp.
"""
import math
import struct


def _f32(value):
    return struct.unpack('>f', struct.pack('>f', value))[0]


def evaluate_float_curve(value, curve, table):
    """82923EA8 float wrapper, restricted to finite normalized inputs.

    Preserve FMULS before FCTIWZ and the original reciprocal's exact bits.
    Invalid unsigned selectors bypass the input and table in the native helper.
    Out-of-domain values for valid selectors are rejected, not clamped.
    """
    if not isinstance(curve, int) or not 0 <= curve <= 0xFFFFFFFF:
        raise ValueError('Curve selector must be an unsigned 32-bit integer')
    if curve > 9:
        return 0.0
    if not math.isfinite(value) or not 0 <= value <= 1:
        raise ValueError('Float curve input must be finite and in 0..1')
    normalized = int(_f32(_f32(value) * 32767.0))
    level = evaluate_curve(normalized, curve, table)
    # Base-disc 822F3538 = 0x38000100; do not use a binary64 reciprocal.
    return _f32(float(level) * _f32(1.0 / 32767.0))


def read_curve_table(data):
    if len(data) != 513 * 4:
        raise ValueError('MixMap curve table must contain exactly 513 big-endian words')
    return struct.unpack('>513i', data)


def evaluate_curve(value, curve, table):
    if not 0 <= value <= 32767:
        raise ValueError('Curve input must be in the verified range 0..32767')
    if not 0 <= curve <= 9:
        raise ValueError('Curve selector must be in 0..9')
    if len(table) != 513 or any(not 0 <= v <= 32767 for v in table):
        raise ValueError('Curve table must contain 513 levels in 0..32767')

    def run(x, selector):
        if selector in (1, 3, 5, 7, 9):
            return run(32767 - x, selector - 1)
        if selector == 8:
            return 32767 - x
        if selector in (2, 6):
            level = run(x, selector - 2)
            return (level * level) >> 15
        # RLWIMI shift=9, MB=18, ME=21 inserts only bits 10..13.
        weight = 0x3FF | ((x << 9) & 0x3C00)
        index = x >> 6
        if selector == 0:
            if index >= 511:
                return 0
            first = table[index]
            if first == 0:
                return 0
            second = table[index + 1]
        else:  # Selector 4 reverses the lookup and complements both endpoints.
            index = 511 - index
            first, second = 32767 - table[index], 32767 - table[index + 1]
        return first + (((second - first) * weight) >> 15)

    return run(value, curve)
