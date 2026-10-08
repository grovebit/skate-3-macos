"""Authored inputs to base-disc body/material classifier 82484EC8.

Export only: runtime selection and special-mode overrides are not ported. These scalars must not be compared to host solver impulses.
"""
import math
import struct

from tools.asset_pipeline.vlt import hash64


CLASS = 'aud_intensitymapping'
REFERENCE = 'Hash_E228508FE0F53970'
# Schema offsets +0C, +08, +00, +04 respectively. 82485024..824850AC
# rejects strength < minimum, then uses strict > for medium and hard.
FIELDS = {
    'minimum': 'Hash_D660AC459139BDF4',
    'medium': 'Hash_7D8DEDD338D45482',
    'hard': 'Hash_C8DED1BC20B9D6A5',
    'maximum': 'Hash_B8870D2001033E0F',
}


def intensity_tables(rows):
    """Retain ordinary material references and their complete intensity rows.

    RefSpec's collection hash names the table; neither the material name nor
    the parent name is a substitute (e.g. head references face_head).
    """
    return material_tables(rows, CLASS, REFERENCE, FIELDS, 'EA::Reflection::Float', '>f')


def material_tables(rows, class_name, reference_field, fields, scalar_type, scalar_format, *, allow_null=False):
    """Resolve material RefSpecs and preserve typed authored scalar tables."""
    mappings = {}
    by_hash = {}
    for row in rows:
        if row['class'] != class_name:
            continue
        key = row['key']
        identity = hash64(key)
        if identity in by_hash:
            raise ValueError(f'Duplicate mapping collection: {key}')
        values = {}
        for name, field in fields.items():
            value = row['fields'][field]
            raw = bytes.fromhex(value['data'])
            if value['type'] != scalar_type or len(raw) != 4:
                raise ValueError(f'{key}: unsupported mapping scalar {field}')
            number, = struct.unpack(scalar_format, raw)
            if not math.isfinite(number) or number < 0:
                raise ValueError(f'{key}: invalid mapping scalar {field}')
            values[name] = number
        # Retain authored boundaries exactly; do not sort or normalize them.
        mappings[key] = values
        by_hash[identity] = key

    materials = {}
    for row in rows:
        if row['class'] != 'aud_material':
            continue
        key = row['key']
        if key in materials:
            raise ValueError(f'Duplicate audio material: {key}')
        reference = row['fields'][reference_field]
        raw = bytes.fromhex(reference['data'])
        if reference['type'] != 'Attrib::RefSpec' or len(raw) != 24:
            raise ValueError(f'{key}: unsupported mapping reference')
        cls, target = struct.unpack_from('>QQ', raw)
        if allow_null and cls == hash64(class_name) and target == 0:
            materials[key] = None
            continue
        if cls != hash64(class_name) or target not in by_hash:
            raise ValueError(f'{key}: unresolved mapping reference')
        materials[key] = by_hash[target]
    if not materials:
        raise ValueError('No audio material mapping references')
    return {'reference_field': reference_field, 'fields': fields.copy(),
            'materials': materials, 'tables': mappings}
