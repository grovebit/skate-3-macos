"""Embedded SNR streams in Skate 3 Xbox 360 SPLC v3 collision banks.

The control section length at 0x08 is relative to the 0x3c-byte header.
A 12-byte record per stream (count at 0x18) follows that section. Each record
contains a relative start, an encoded-data end and a source hash. Starts are
relative to the payload block after the table. The next start, not the encoded
end, bounds the complete SNR (which includes codec framing).

The stream reader preserves raw samples. The event reader also resolves the
observed leaf/group/layer layout, without reconstructing the original mixer.
"""
from dataclasses import dataclass
import struct


@dataclass(frozen=True)
class Stream:
    offset: int
    size: int
    source_hash: int


def streams(data: bytes) -> list[Stream]:
    if len(data) < 0x3c or data[:8] != b'SPLC\0\0\0\3':
        raise ValueError('Expected an SPLC version 3 bank')
    control_size = struct.unpack_from('>I', data, 8)[0]
    count = struct.unpack_from('>I', data, 0x18)[0]
    table = 0x3c + control_size
    payload = table + count * 12
    if not 1 <= count <= 4096 or payload > len(data):
        raise ValueError('Invalid SPLC stream table bounds')
    records = [struct.unpack_from('>III', data, table + i * 12) for i in range(count)]
    if records[0][0] != 0:
        raise ValueError('SPLC first stream must start at the payload block')
    result = []
    for index, (start, encoded_end, identity) in enumerate(records):
        stop = records[index + 1][0] if index + 1 < count else len(data) - payload
        if not 0 <= start < encoded_end < stop <= len(data) - payload:
            raise ValueError(f'Invalid SPLC stream {index + 1} bounds')
        offset = payload + start
        # Only the observed 48 kHz mono XMA2 SNR variant is supported here.
        if stop - start < 16 or data[offset:offset + 4] != b'\x03\x00\xbb\x80':
            raise ValueError(f'Unsupported SPLC stream {index + 1} codec/header')
        result.append(Stream(offset, stop - start, identity))
    return result


@dataclass(frozen=True)
class PanChoice:
    sample: int
    offset: int
    angle_degrees: float
    pitch_multiplier: float


@dataclass(frozen=True)
class Choice:
    sample: int
    offset: int
    gain: float
    source_rate: float
    pitch_multiplier: float
    angle_degrees: float
    delay: float
    source_start: float
    source_end: float
    attack_end: float
    release_start: float
    envelope_curve: int
    gain_randomization: float
    source_rate_range: float
    delay_range: float
    probability: float
    routing_flag: int
    effect_index: int
    priority: int


@dataclass(frozen=True)
class Layer:
    offset: int
    selection_mode: int
    initial_state: int
    choices: list[Choice]


@dataclass(frozen=True)
class Leaf:
    offset: int
    gain: float
    source_rate: float
    source_rate_range: float
    layers: list[Layer]


@dataclass(frozen=True)
class Event:
    offset: int
    selection_mode: int | None
    initial_state: int | None
    variants: list[Leaf]


def event_controls(data: bytes) -> list[Event]:
    """Decode established fields without interpreting unknown bytes.

    Selection: 8294DCF0/8294DDF8; gain/rate: 8294E070/8294E230;
    delay: 8294E32C; envelope: 8294F258. Times retain authored float32
    values. The source-rate fields are distinct from choice +0C pitch.
    """
    sample_count = len(streams(data))
    control_size, leaves, groups, reserved = struct.unpack_from('>4I', data, 8)
    end = 0x3c + control_size
    group_start = 0x3c + leaves * 36
    cursor = group_start + groups * 72
    if reserved or not 0 < leaves <= 4096 or groups > 4096 or cursor > end:
        raise ValueError('Unsupported SPLC control layout')
    result = []
    for index in range(leaves):
        kind, identity, layers = struct.unpack_from('>IHH', data, 0x3c + index * 36)
        if kind or identity != index or layers > 32:
            raise ValueError('Unsupported SPLC leaf')
        variant = []
        leaf_offset = 0x3c + index * 36
        for _ in range(layers):
            if cursor + 12 > end:
                raise ValueError('Truncated SPLC layer')
            header = data[cursor:cursor + 12]
            count = header[8]
            if (header[:8] != b'\0\0\0\0\0\1\0\0' or header[9] > 2
                    or header[10:] != b'\0\0' or not 1 <= count <= 32
                    or cursor + 12 + count * 72 > end):
                raise ValueError('Unsupported SPLC sample choices')
            samples = []
            for choice in range(count):
                sample, = struct.unpack_from('>H', data, cursor + 12 + choice * 72)
                if sample >= sample_count:
                    raise ValueError('SPLC choice refers to a missing stream')
                offset = cursor + 12 + choice * 72
                values = struct.unpack_from('>9f', data, offset + 4)
                randomization = struct.unpack_from('>3f', data, offset + 0x2c)
                probability, = struct.unpack_from('>f', data, offset + 0x40)
                samples.append(Choice(sample + 1, offset, *values,
                                      data[offset + 0x28] & 15, *randomization,
                                      probability, data[offset + 0x44], data[offset + 3],
                                      struct.unpack_from('>b', data, offset + 0x3c)[0]))
            state, = struct.unpack_from('>I', data, cursor + 4)
            variant.append(Layer(cursor, header[9], state, samples))
            cursor += 12 + count * 72
        gain, rate, rate_range = struct.unpack_from('>3f', data, leaf_offset + 8)
        result.append(Event(leaf_offset, None, None,
                            [Leaf(leaf_offset, gain, rate, rate_range, variant)]))
    if cursor != end:
        raise ValueError('Unconsumed SPLC control data')
    for index in range(groups):
        record = group_start + index * 72
        kind, = struct.unpack_from('>I', data, record)
        children = struct.unpack_from('>32H', data, record + 4)
        count, mode, a, b = data[record + 68:record + 72]
        if (kind != 0x10000 or not 1 <= count <= 32 or mode > 2 or a or b
                or any(child >= leaves for child in children[:count])
                or any(child != 0xffff for child in children[count:])):
            raise ValueError('Unsupported SPLC event group')
        result.append(Event(record, mode, kind,
                            [result[child].variants[0] for child in children[:count]]))
    return result


def event_pan_choices(data: bytes) -> list[list[list[list[PanChoice]]]]:
    """Preserve the existing pan inspector's output and choice ordering."""
    return [[[[PanChoice(c.sample, c.offset, c.angle_degrees, c.pitch_multiplier)
               for c in layer.choices] for layer in leaf.layers]
             for leaf in event.variants] for event in event_controls(data)]


def event_layers(data: bytes) -> list[list[list[list[int]]]]:
    """Resolve sample choices, deduplicated for the existing offline exporter.

    Use event_pan_choices to retain authored choices with distinct pan angles.
    """
    return [[[list(dict.fromkeys(c.sample for c in layer.choices))
              for layer in leaf.layers] for leaf in event.variants]
            for event in event_controls(data)]


def event_samples(data: bytes) -> list[list[int]]:
    """Flatten resolved events for sample audition, retaining first-use order."""
    return [list(dict.fromkeys(sample for variant in event for layer in variant for sample in layer))
            for event in event_layers(data)]
