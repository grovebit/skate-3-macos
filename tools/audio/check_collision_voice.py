"""Evaluate recovered collision gain from explicit original-controller inputs.

Base-disc 8249D138 reads packed controller values; 824C0334..824C03DC
combines levels. This reference checker does not recover the controller's
producer or establish audible parity. Words are numeric u32s after big-endian
decoding, not a host-endian byte dump. Omitted words model a missing controller.
"""
import argparse
import json
import struct

NORMALIZATION = struct.unpack('>f', bytes.fromhex('38000100'))[0]


def f32(value):
    return struct.unpack('>f', struct.pack('>f', value))[0]


def binding_key(controller_id):
    """82473690: routing key used before attaching the controller owner.

    This decodes lookup inputs only; it does not claim a live object exists.
    """
    if not 0 <= controller_id <= 0xFFFFFFFF:
        raise ValueError('Controller ID must be an unsigned 32-bit integer')
    family = (controller_id >> 16) & 0xFF
    return {
        'family': family,
        'manager_slot_offset': (0xA0 + family) * 4,
        'group': (controller_id >> 11) & 0x1F,
        'object': (controller_id >> 4) & 0x7F,
        'object_list_offset': 0x24 if controller_id & 0xE0000000 == 0x40000000 else 0x20,
    }


def controller_level(words, slot):
    """8249D138: low/high halfword, discard bit15; null owner returns zero."""
    if slot < 0:
        raise ValueError('Controller slot must be nonnegative')
    if words is None:
        return 0
    index = slot >> 1
    if index >= len(words):
        raise ValueError('Controller capture does not contain the requested slot')
    word = words[index]
    if not 0 <= word <= 0xFFFFFFFF:
        raise ValueError('Controller word must be an unsigned 32-bit integer')
    return (word >> (16 * (slot & 1))) & 0x7FFF


def collision_gain(queued_level, authored_base_level, words, slot):
    """Preserve each single-precision multiply and both truncation stages."""
    for value in (queued_level, authored_base_level):
        if not -(1 << 31) <= value < (1 << 31):
            raise ValueError('Levels must be signed 32-bit integers')
    control = controller_level(words, slot)
    base = min(32767, max(0, authored_base_level))
    stage1 = int(f32(f32(f32(queued_level) * NORMALIZATION) * f32(control)))
    if not -(1 << 31) <= stage1 < (1 << 31):
        raise ValueError('First conversion is outside the verified signed integer range')
    stage2 = int(f32(f32(stage1) * f32(f32(base) * NORMALIZATION)))
    if not -(1 << 31) <= stage2 < (1 << 31):
        raise ValueError('Second conversion is outside the verified signed integer range')
    return {'controller_level': control, 'base_level': base,
            'after_controller': stage1, 'after_material': stage2,
            'voice_gain': f32(f32(stage2) * NORMALIZATION)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    number = lambda text: int(text, 0)
    parser.add_argument('--level', type=number, required=True)
    parser.add_argument('--base-level', type=number, required=True)
    parser.add_argument('--slot', type=number, required=True)
    parser.add_argument('--controller-id', type=number,
                        help='Optional controller ID to decode its owner lookup key')
    parser.add_argument('--controller-words', type=number, nargs='+',
                        help='Numeric u32 words; omission models a missing controller')
    args = parser.parse_args()
    try:
        result = collision_gain(args.level, args.base_level, args.controller_words, args.slot)
        if args.controller_id is not None:
            result['binding_key'] = binding_key(args.controller_id)
    except ValueError as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
