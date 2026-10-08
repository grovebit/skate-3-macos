"""Authored wheel-contact tables used by base-disc 824A8288/824AC918."""
import math
import struct
from tools.owned_game.audio_surface import surface_entries


FIELDS = [
    ('5A93802D11B00173', 'CEA5AFA8BA170B07'),
    ('A0F86FEEA9C2412F', '951AD53718030327'),
    ('797EC34502499EC3', '068C8C5EBEC1B45F'),
    ('AB0D92058B4293A7', '6823F4910A1882AD'),
]


def contact_routes(rows, events, select_event):
    def fields(cls, key):
        return next(r['fields'] for r in rows if r['class'] == cls and r['key'] == key)

    contacts = fields('aud_contacts', 'default')

    def number(value):
        result = struct.unpack('>f', bytes.fromhex(value))[0]
        if not math.isfinite(result) or result < 0:
            raise ValueError('Invalid contact audio scalar')
        return result

    thresholds = [number(contacts['Hash_' + key]['data'])
                  for key in ('2A70BB8A382574E4', '6D3D91A9BA7ADCDC')]
    if not 0 < thresholds[0] < thresholds[1] <= 1:
        raise ValueError('Unsupported contact category thresholds')
    if bytes.fromhex(contacts['Hash_642CF9BFEC6BE988']['data'])[0] != 1:
        raise ValueError('Packed contact-category mode is not implemented')
    modes, volumes = [], []
    for mode, (event_key, volume_key) in enumerate(FIELDS):
        event_field = contacts['Hash_' + event_key]
        ids = event_field['array']['items']
        gains = contacts['Hash_' + volume_key]['array']['items']
        if event_field['type'] != 'Skate_Collisions' or len(ids) != 13 or len(gains) != 15:
            raise ValueError('Unsupported original contact table layout')
        routes = []
        for index, value in enumerate(ids):
            event = int(value, 16)
            if event >= len(events['Skate_Collisions']):
                raise ValueError('Contact event outside Skate_Collisions')
            variants = events['Skate_Collisions'][event]
            if not any(variants):
                if mode == 0 or index not in (2, 5, 8):
                    raise ValueError('Missing required wheel-contact event')
                routes.append(None)
            else:
                routes.append(select_event('Skate_Collisions', event, 'Hash_' + event_key,
                                           f'wheel-contact-{mode}-{index}', events))
        modes.append(routes)
        volumes.append([number(value) for value in gains])
    # 82482BB8: read word +8, using entry 94 for out-of-range surfaces.
    surfaces = [struct.unpack_from('>I', entry, 8)[0] != 0 for entry in surface_entries(rows)]
    return {'thresholds': thresholds, 'surface_modes': surfaces,
            'modes': modes, 'volumes': volumes,
            'event_ids': [[int(v, 16) for v in contacts['Hash_' + key]['array']['items']]
                          for key, _ in FIELDS]}
