"""Authored map starts: the retail start locator, position and heading of a district.

Evidence is in docs/world/spawns.md. In the base-disc executable, 826C77E0
copies a skate park's world-row start locator, read only when the world's
park flag is set, into the session's start-locator name. 82872D80/82872F00
resolve that name through the locator manager (828E9598), and 828E9688 hands
player slot 0 the full matrix of the EB0009 record's first spawn slot, or of
the record itself when it has no slots. Other districts keep a name set
elsewhere: the new-game fe_locations rows gamestart/schoolstart (825F1600) or
another setter whose callers are not traced (8283F758).
"""
import json
import math
import struct
import tempfile
from pathlib import Path

from tools.owned_game.big import BigArchive
from .dynamic_props import matrix, sections
from .environment import Collections, key_hash
from .teleports import cstring, location_records
from .vlt import vault

PARK = key_hash('Hash_0C54AD24155E3220')
START_LOCATOR = key_hash('Hash_3735C5C12E8E7AE1')
# Districts without a park start whose fe_locations rows name several
# locations. Retail picks one per session, but a map needs one fixed start.
# University's gamestart is the executable's new-game and fallback start
# (825F1600, 826BAFC8); the other three are provisional project choices.
DEFAULT_LOCATIONS = {
    'DIST_University': 'gamestart',
    'DIST_Industrial': 'dist_industrial_reclaimed',  # Haystings Park
    'DIST_DownTown': 'dist_downtown_skatepark',  # Rosalita Skate Park
    'DIST_MaloofMoneyCup': 'dist_maloof_street',  # street course
}


def heading(forward):
    """Yaw for the game's Mat3::from_rotation_y basis: z column (sin h, 0, cos h)."""
    return math.atan2(forward[0], forward[2])


def stream(collections, world_key):
    value = collections.resolve('world', world_key)[0].get(key_hash('WorldStream'))
    return value['data'] if value else None


def park_start(collections, district):
    """(locator, source) from the district's root world row, or None if it is no park.

    Mode variants (full, empty, tutorials) inherit from the root row; only
    StartPark's tutorial sign-up variant names a different locator."""
    rows = [row for (cls, key), row in collections.rows.items()
            if cls == key_hash('world') and stream(collections, key) == district]
    roots = [row for row in rows if key_hash(row['parent']) not in {key_hash(r['key']) for r in rows}]
    if len(roots) != 1:
        raise ValueError(f'{district} has {len(roots)} root world rows')
    fields, _ = collections.resolve('world', roots[0]['key'])
    if not (PARK in fields and bytes.fromhex(fields[PARK]['data'])[0]):
        return None
    if START_LOCATOR not in fields or not fields[START_LOCATOR]['data']:
        raise ValueError(f"Park world {roots[0]['key']} has no start locator")
    return fields[START_LOCATOR]['data'], 'world ' + roots[0]['key']


def collection_strings(game_root):
    """The skatercollections string table that fe_locations `location` offsets index."""
    database = BigArchive(Path(game_root) / 'data/big/db.big')
    with tempfile.TemporaryDirectory(prefix='skate-map-start-') as tmp:
        entries = [e for e in database.entries if e.path in
                   ('data/db/skatercollections.bin', 'data/db/skatercollections.vlt')]
        database.extract_entries(entries, Path(tmp))
        return vault(Path(tmp) / 'data/db/skatercollections')[1]


def menu_start(collections, binary, district):
    """(locator, source) from the district's fe_locations rows."""
    rows = {}
    for (cls, key), row in collections.rows.items():
        if cls != key_hash('fe_locations'):
            continue
        fields, _ = collections.resolve(cls, key)
        world = fields.get(key_hash('World'))
        if world and stream(collections, int(world['data'][:16], 16)) == district:
            rows[row['key']] = cstring(binary, int(fields[key_hash('location')]['data'], 16)).decode('utf-8')
    if district in DEFAULT_LOCATIONS:
        key = DEFAULT_LOCATIONS[district]
        if key not in rows:
            raise ValueError(f'Chosen start {key} is not an fe_locations row of {district}')
        return rows[key], 'fe_locations ' + key
    if len(set(rows.values())) != 1:
        raise ValueError(f'{district} has {len(set(rows.values()))} fe_locations starts and no chosen one')
    return next(iter(rows.values())), 'fe_locations ' + ', '.join(sorted(rows))


def first_slot(raw, record):
    """(name, matrix, offset) where 828E9688 places player 0 for this record.

    An EB0009 record may own per-player spawn slots: +0x60 counts 0x70-byte
    entries (a matrix, then a name pointer at +0x60) and +0x64 points at the
    first, relative to the section, inside the slot array after the record
    table. Player 0 takes the first slot, or the record's own matrix when
    there are none."""
    at = record['source_offset']
    count, pointer = struct.unpack_from('>2I', raw, at + 0x60)
    if not count:
        return record['locator'], record['matrix'], at
    base, size = next((offset, size) for offset, _, size, _, _, kind in sections(raw)
                      if kind == 0xEB0009 and offset <= at < offset + size)
    slots = struct.unpack_from('>I', raw, base + 16)[0]
    if pointer < slots or (pointer - slots) % 0x70 or pointer + count * 0x70 > size:
        raise ValueError(f"Invalid spawn slots of locator {record['locator']}")
    slot = base + pointer
    name = cstring(raw, base + struct.unpack_from('>I', raw, slot + 0x60)[0], base + size).decode('utf-8')
    return name, matrix(raw, slot).tolist(), slot


def start(game_root, converted, district):
    """{locator, source, record, position, heading} of the district's authored start.

    `locator` and `record` name the slot or record whose matrix is used."""
    game_root = Path(game_root)
    collections = Collections(converted)
    locator, source = (park_start(collections, district)
                       or menu_start(collections, collection_strings(game_root), district))
    matches = {}
    for path in sorted((game_root / 'data/content/global_locators').rglob('*.rx2')):
        if path.parent.name.casefold() == district.casefold():
            raw = path.read_bytes()
            for record in location_records(raw):
                if record['locator'] == locator:
                    name, placed, at = first_slot(raw, record)
                    where = f"{path.relative_to(game_root).as_posix()}+{at:#x}"
                    matches.setdefault(json.dumps(placed), (name, placed, where))
    if len(matches) != 1:
        raise ValueError(f'{district} start locator {locator} has {len(matches)} distinct placements')
    name, placed, where = next(iter(matches.values()))
    return dict(locator=name, source=source, record=where,
                position=[float(v) for v in placed[3][:3]], heading=heading(placed[2]))
