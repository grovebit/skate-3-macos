"""Audio surface mapping shared by wheel routing and collision class selection."""
import struct

SURFACE_MAP = 'Hash_4CA607558B1CF440'


def surface_entries(rows):
    mappings = [r for r in rows if r['class'] == 'aud_general' and r['key'] == 'mapping']
    if len(mappings) != 1:
        raise ValueError('Expected one audio surface mapping')
    array = mappings[0]['fields'][SURFACE_MAP]['array']
    if array['element_size'] != 72 or len(array['items']) != 95:
        raise ValueError('Unsupported audio surface map')
    entries = [bytes.fromhex(item) for item in array['items']]
    if any(len(entry) != 72 for entry in entries):
        raise ValueError('Invalid audio surface entry size')
    return entries


def surface_classes(rows):
    # 8248581C..82485884: surface mapping entry +1C, not the wheel-mode +08.
    classes = [struct.unpack_from('>i', entry, 0x1c)[0] for entry in surface_entries(rows)]
    if any(value not in (0, 1, 2) for value in classes):
        raise ValueError('Unsupported audio counterpart class')
    return classes
