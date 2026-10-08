"""Primary-body event selection from base-disc 82484638..824849C4.

The primary body IDs 61..64 use bank 0 (Skate_Collisions), verified against
82FD1930. Other bank branches and special-mode overrides are not covered here.
"""

# Rows: soft, medium, hard. Columns: counterpart classes 0, 1, 2.
# These are event fields, not the volume profile's counterpart-class rules.
FIELDS = (
    ('BFABF634D2B1E45A', '9ABFC64574AB2F9F', 'BCD5E888294F7B15'),
    ('EF9BD81F9CFF725F', 'A3ADCA7B19287B5D', 'C676C87F862C0490'),
    ('9203DF6FD029B377',) * 3,
)
PARTS = ('head', 'torso', 'arm', 'leg')


def body_event_routes(rows, events, select_event):
    materials = {}
    for row in rows:
        if row['class'] == 'aud_material' and row['key'] in PARTS:
            if row['key'] in materials:
                raise ValueError(f"Duplicate body material: {row['key']}")
            materials[row['key']] = row
    if set(materials) != set(PARTS):
        raise ValueError('Missing primary body material')
    result = {}
    for part in PARTS:
        categories = []
        for fields in FIELDS:
            routes = []
            for field in fields:
                key = 'Hash_' + field
                value = materials[part]['fields'][key]
                raw = bytes.fromhex(value['data'])
                if value['type'] != 'Skate_Collisions' or len(raw) != 4:
                    raise ValueError(f'{part}: invalid body event field {key}')
                event = int.from_bytes(raw, 'big')
                # Event 0 is a valid bank event, not an absent reference.
                routes.append(select_event('Skate_Collisions', event, key, part, events))
            categories.append(routes)
        result[part] = categories
    return result
