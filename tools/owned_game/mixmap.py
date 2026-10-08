"""Read verified declarations and output routes in Skate 3 MixMapSK8.mxb.

Base-disc 82925690 locates groups. 8292A6A8 reads the +04 section and
8292AD44..8292AD5C advances variable-length declarations. Other sections,
expression semantics, execution and controller publication remain unported.
"""
import hashlib
import struct


def _words(data, offset, count):
    if offset < 0 or offset % 4 or count < 0 or count > (len(data) - offset) // 4:
        raise ValueError(f'MixMap word range outside file: {offset:#x}, count {count}')
    return struct.unpack_from(f'>{count}I', data, offset)


def _signed(word):
    return word if word < 0x80000000 else word - 0x100000000


def _directory(data):
    header = _words(data, 0, 4)
    count = _signed(header[1])
    if count < 0:
        raise ValueError('Negative MixMap group count')
    return header[2], _words(data, header[2], count)


def declarations(data):
    directory_offset, directory = _directory(data)
    groups = []
    for index, offset in enumerate(directory):
        if offset == 0xFFFFFFFF:
            groups.append({'index': index, 'offset': None, 'section_offset': None,
                           'declarations': []})
            continue
        # Only +04 is decoded. Do not infer meanings for the other group words.
        relative = _signed(_words(data, offset, 2)[1])
        records = []
        section = None
        if relative >= 0:
            section = offset + relative
            section_header = _words(data, section, 4)
            record_count = _signed(section_header[0])
            if record_count < 0:
                raise ValueError('Negative MixMap declaration count')
            cursor = section + 16
            # Every record needs at least two words; bound the loop first.
            if record_count > (len(data) - cursor) // 8:
                raise ValueError('MixMap declaration count exceeds file')
            for _ in range(record_count):
                identity, descriptor = _words(data, cursor, 2)
                argument_count = (descriptor >> 16) & 0x1F
                arguments = _words(data, cursor + 8, argument_count)
                records.append({'offset': cursor, 'id': f'{identity:08X}',
                                'descriptor': f'{descriptor:08X}',
                                'arguments': [f'{v:08X}' for v in arguments]})
                cursor += 4 * (2 + argument_count)
        groups.append({'index': index, 'offset': offset, 'section_offset': section,
                       'declarations': records})
    return {'sha256': hashlib.sha256(data).hexdigest(), 'size': len(data),
            'directory_offset': directory_offset, 'groups': groups}


def output_routes(data):
    """Inspect +10 output records and +14 write maps, not evaluated levels.

    Record traversal: 8292AF28..8292B144. Mapping traversal: 8292BAA0..8292BCBC.
    Write fields: 82928590..82928600. Format-dependent arithmetic is unported.
    """
    _, directory = _directory(data)
    groups = []
    for index, offset in enumerate(directory):
        group = {'index': index, 'section_offset': None, 'mapping_offset': None,
                 'buffer_count': None, 'routes': []}
        groups.append(group)
        if offset == 0xFFFFFFFF:
            continue
        header = _words(data, offset, 6)
        relative = _signed(header[4])
        if relative < 0:
            continue
        section = offset + relative
        count, buffers, _, _ = _words(data, section, 4)
        count = _signed(count)
        cursor = section + 16
        if count < 0 or count > (len(data) - cursor) // 12:
            raise ValueError('Invalid MixMap output record count')
        group.update(section_offset=section, buffer_count=buffers)
        if not count:
            continue
        mapping_relative = _signed(header[5])
        if mapping_relative < 0:
            raise ValueError('Missing MixMap write map for output records')
        mapping = offset + mapping_relative
        group['mapping_offset'] = mapping
        for _ in range(count):
            descriptor, value, owner = _words(data, cursor, 3)
            argument_count = (descriptor >> 16) & 0xFF
            arguments = _words(data, cursor + 12, argument_count)
            mapping_header, = _words(data, mapping, 1)
            # Traversal uses eight bits; execution uses only five. Preserve both.
            writes = _words(data, mapping + 4, mapping_header & 0xFF)
            group['routes'].append({
                'offset': cursor, 'descriptor': f'{descriptor:08X}',
                'value_word': f'{value:08X}', 'controller_id': f'{owner:08X}',
                'arguments': [f'{v:08X}' for v in arguments],
                'mapping_offset': mapping, 'mapping_header': f'{mapping_header:08X}',
                'format': (mapping_header >> 24) & 0xF,
                'write_count': mapping_header & 0x1F,
                'writes': [{'word': f'{v:08X}', 'slot': (v >> 26) & 0x1F,
                            'argument_index': (v >> 21) & 0x1F,
                            'low_i16': (v & 0x7FFF) - (v & 0x8000)} for v in writes],
            })
            cursor += 4 * (3 + argument_count)
            mapping += 4 * (1 + len(writes))
    return groups


def modulation_routes(data):
    """Read +08 modulation records and their authored mode blocks.

    8292B680 traverses 4 + 24 * (header's high byte & 15) bytes per node.
    82929F00 selects a mode, falling back to mode zero when no match exists.
    This reports inputs and bounds, not the spatial producer or curve output.
    Controller IDs here are for instance zero; instantiation inserts group bits.
    A null input slot means the selector uses a constant (-1.0 for the float,
    zero for phase), not an unresolved binding (8292A074..8292A0B8).
    """
    _, directory = _directory(data)
    groups = []
    for index, offset in enumerate(directory):
        group = {'index': index, 'section_offset': None, 'nodes': []}
        groups.append(group)
        if offset == 0xFFFFFFFF:
            continue
        relative = _signed(_words(data, offset, 3)[2])
        if relative < 0:
            continue
        section = offset + relative
        count = _signed(_words(data, section, 4)[0])
        cursor = section + 16
        if count < 0 or count > (len(data) - cursor) // 4:
            raise ValueError('Invalid MixMap modulation record count')
        group['section_offset'] = section
        for node_index in range(count):
            header, = _words(data, cursor, 1)
            mode_count = (header >> 24) & 0xF
            modes = []
            for mode_index in range(mode_count):
                at = cursor + 4 + mode_index * 24
                mode, curves, *bounds = _words(data, at, 6)
                float_selector = (mode >> 12) & 0xF
                phase_selector = (mode >> 8) & 0xF
                modes.append({
                    'offset': at, 'mode_word': f'{mode:08X}',
                    'mode': (mode >> 24) & 0xF, 'curve_word': f'{curves:08X}',
                    'float_selector': float_selector,
                    'float_input_slot': {0: 1, 1: 0}.get(float_selector),
                    'phase_selector': phase_selector,
                    'phase_input_slot': {0: 3, 1: 2}.get(phase_selector),
                    'bounds_words': [f'{v:08X}' for v in bounds],
                    'bounds': [[v & 0x7FFF, (v >> 16) & 0x7FFF] for v in bounds],
                })
            # B8D4..B8E8 creates a type-60000000 input reference; the controller
            # resolver strips flag bits and low selector bits (8292797C..82927988).
            controller_id = (0x60000000 | (header & 0x1FFFFFFF)) & 0xE0FFFFF0
            group['nodes'].append({'index': node_index, 'offset': cursor,
                                   'header': f'{header:08X}',
                                   'controller_id': f'{controller_id:08X}',
                                   'modes': modes})
            cursor += 4 + mode_count * 24
    return groups
