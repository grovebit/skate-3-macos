"""Reference for the level outputs of 82929F00's active spatial modulation node.

Inputs are an explicit original distance and packed phase, not host defaults.
Mode selection, producer lifetime, and the auxiliary pitch path remain outside
this helper. Normalization preserves f32 operations; curve and log tables must
come from the caller's owned executable. Nonfinite/degenerate inputs are outside
the verified reference domain and rejected instead of inventing fallback rules.
"""
import math
import struct

from tools.owned_game.mixmap_curve import _f32, evaluate_curve


def read_log_table(data):
    if len(data) != 512 * 4:
        raise ValueError('MixMap log table must contain exactly 512 big-endian words')
    return struct.unpack('>512i', data)


def linear_to_log(level, table):
    """8292A264..8292A450, including sparse-bin upper-end selection."""
    if not 0 <= level <= 32767:
        raise ValueError('Linear level must be in 0..32767')
    if len(table) != 512 or any(not 0 <= v <= 601 for v in table):
        raise ValueError('MixMap log table must contain 512 values in 0..601')
    if level == 0:
        return -10000
    exponent = level.bit_length() - 1
    remainder = level - (1 << exponent)
    if exponent >= 9:
        index = remainder >> (exponent - 9)
    else:
        shift = 9 - exponent
        index = (remainder << shift) + (1 << shift) - 1
    return table[index] - 602 * (15 - exponent)


def evaluate_node(distance, phase, bounds, curve_word, curve_table, log_table,
                  *, enabled=True):
    """Return only the linear/log level pair, not the node's complete state.

    8292A0B8..8292A25C chooses and blends the directional curves. The disabled
    and beyond-both-upper-bounds branches both produce silence for this pair.
    """
    if not enabled:
        return {'linear_level': 0, 'log_level': -10000}
    if not math.isfinite(distance) or abs(distance) > 3.4028234663852886e38:
        raise ValueError('Distance must be a finite f32 value')
    if not 0 <= phase <= 65535:
        raise ValueError('Packed phase must be in 0..65535')
    if len(bounds) != 4 or any(len(pair) != 2 or not 0 <= pair[0] < pair[1] <= 32767
                               for pair in bounds):
        raise ValueError('Four strictly increasing 15-bit bound pairs are required')
    if not 0 <= curve_word <= 0xFFFFFFFF:
        raise ValueError('Curve word must be an unsigned 32-bit value')
    distance = _f32(distance)
    quadrant = phase >> 14
    remainder = phase - (quadrant << 14)
    first, second = bounds[quadrant], bounds[(quadrant + 1) & 3]
    # The second and third quadrant nibbles are deliberately not sequential.
    selector = (curve_word >> (28, 16, 24, 20)[quadrant]) & 15
    if distance > first[1] and distance > second[1]:
        return {'linear_level': 0, 'log_level': -10000}

    def curve(pair):
        low, high = pair
        clamped = min(high, max(low, distance))
        fraction = _f32(_f32(clamped - low) / _f32(high - low))
        normalized = int(_f32(fraction * 32767.0))
        # The native caller returns zero for selectors above nine.
        return evaluate_curve(normalized, selector, curve_table) if selector <= 9 else 0

    level1 = curve(first)
    level2 = curve(second) if remainder else 32767
    weight = remainder * 2
    linear = ((level1 * (32767 - weight)) >> 15) + ((level2 * weight) >> 15)
    return {'linear_level': linear, 'log_level': linear_to_log(linear, log_table)}
