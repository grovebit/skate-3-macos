"""Original shared-sum binding and evaluation, with explicit runtime inputs.

8292B928 expands sum references; 8292BC20 uses the same rule for non-modulation
output arguments. 829283DC..82928458 accumulates and clamps resolved values.
This does not discover live instance counts or evaluate referenced controllers.
"""


def expand_references(references, family, instance, instance_counts):
    """Preserve authored order, replacing bits 11..15 before reference lookup.

    Same-family references select the containing instance. Other families expand
    in ascending instance order. Counts must be supplied, never assumed to be 1.
    For output records, pass only their non-modulation arguments.
    """
    if not isinstance(family, int) or not 0 <= family <= 255:
        raise ValueError('Containing family must be in 0..255')
    if not isinstance(instance, int) or not 0 <= instance <= 31:
        raise ValueError('Containing instance must be in 0..31')
    result = []
    for reference in references:
        if not isinstance(reference, int) or not 0 <= reference <= 0xFFFFFFFF:
            raise ValueError('References must be unsigned 32-bit integers')
        target_family = (reference >> 16) & 255
        base = reference & 0xFFFF07FF  # RLWINM SH=0, MB=21, ME=15.
        if target_family == family:
            instances = (instance,)
        else:
            count = instance_counts.get(target_family)
            if not isinstance(count, int) or not 0 <= count <= 32:
                raise ValueError(f'Missing or unsupported instance count for family {target_family}')
            instances = range(count)
        result.extend(base | (i << 11) for i in instances)
    return tuple(result)


def evaluate_sum(levels, clamp_word, previous=0):
    """Evaluate captured resolved levels, or retain state for a null array.

    None means a null runtime pointer. An empty array is a present allocation
    with zero entries. The original executes only the low byte of the resolved
    reference count, even when binding expanded it beyond 255.
    """
    if not isinstance(previous, int) or not -(1 << 31) <= previous < (1 << 31):
        raise ValueError('Previous sum must be a signed 32-bit integer')
    if levels is None:
        return previous
    if not isinstance(clamp_word, int) or not 0 <= clamp_word <= 0xFFFFFFFF:
        raise ValueError('Clamp word must be an unsigned 32-bit integer')
    total = 0
    for level in levels[:len(levels) & 255]:
        if not isinstance(level, int) or not -(1 << 31) <= level < (1 << 31):
            raise ValueError('Resolved levels must be signed 32-bit integers')
        total = ((total + level + (1 << 31)) & 0xFFFFFFFF) - (1 << 31)
    upper = (clamp_word >> 16) & 0x7FFF
    # The native OR with FFFF0000 forces a negative lower bound, even if
    # the stored low halfword's sign bit is clear; this is not sign-extension.
    lower = (clamp_word & 0xFFFF) - 0x10000
    return max(lower, min(upper, total))
