import math
import struct
import tempfile
import unittest
from pathlib import Path

import numpy as np
from .environment import Collections, key_hash
from .map_starts import DEFAULT_LOCATIONS, heading, menu_start, park_start, start
from .test_dynamic_props import resource

PARK = 'Hash_0C54AD24155E3220'
START = 'Hash_3735C5C12E8E7AE1'


def row(cls, key, parent='', **fields):
    return {'class': cls, 'key': key, 'parent': parent,
            'fields': {name: {'type': '', 'data': value} for name, value in fields.items()}}


def world(key, parent='', **fields):
    return row('world', key, parent, **fields)


def menu(key, world_key, offset):
    return row('fe_locations', key, location=f'{offset:08X}', World=f'{key_hash(world_key):016X}' + '0' * 16)


def locators(*records):
    """One EB0009 section of (name, matrix, *spawn slots) records; a slot is (name, matrix).

    Records, then the 0x70-byte slots of all records, then the names."""
    slots = [slot for _, _, *own in records for slot in own]
    first = 32 + len(records)*128
    names = first + len(slots)*0x70
    payload = bytearray(names)
    struct.pack_into('>6I', payload, 0, len(records), len(slots), 0, 32, first, names)

    def place(at, name, matrix):
        struct.pack_into('>16f', payload, at, *np.ravel(matrix))
        offset = len(payload)
        payload.extend(name.encode() + b'\0')
        return offset
    slot = 0
    for index, (name, matrix, *own) in enumerate(records):
        at = 32 + index*128
        struct.pack_into('>3I', payload, at + 96, len(own), first + slot*0x70 if own else 0, place(at, name, matrix))
        for slot_name, slot_matrix in own:
            struct.pack_into('>I', payload, first + slot*0x70 + 96, place(first + slot*0x70, slot_name, slot_matrix))
            slot += 1
    return resource(0xEB0009, payload)


def placed(position, forward):
    up = np.array([0., 1., 0.])
    side = np.cross(up, forward)
    return np.array([[*side, 0], [*up, 0], [*forward, 0], [*position, 1]])


PARKS = {'collections': [
    world('default', **{PARK: '00000000'}),
    world('default_skateparks', 'default', **{PARK: '01000000'}),
    world('dist_park', 'default_skateparks', WorldStream='DIST_Park', **{START: 'Z_Start'}),
    world('dist_park_signup', 'dist_park', **{START: 'signup_locator'}),
    # Without the park flag the executable never reads the start locator.
    world('dist_city', 'default', WorldStream='DIST_City', **{START: 'ignored'}),
]}


class MapStartTests(unittest.TestCase):
    def test_heading_matches_game_rotation_convention(self):
        # skate-game: basis = Mat3::from_rotation_y(heading), forward = (sin h, 0, cos h).
        for forward, expected in [((0, 0, 1), 0.), ((1, 0, 0), math.pi/2), ((-1, 0, 0), -math.pi/2),
                                  ((0.1536, 0, 0.9881), math.atan2(0.1536, 0.9881))]:
            h = heading(forward)
            self.assertAlmostEqual(h, expected, places=6)
            self.assertAlmostEqual(math.sin(h), forward[0]/math.hypot(forward[0], forward[2]), places=6)
            self.assertAlmostEqual(math.cos(h), forward[2]/math.hypot(forward[0], forward[2]), places=6)

    def test_park_start_is_the_root_world_rows_locator(self):
        collections = Collections(PARKS)
        self.assertEqual(park_start(collections, 'DIST_Park'), ('Z_Start', 'world dist_park'))
        self.assertIsNone(park_start(collections, 'DIST_City'))
        with self.assertRaisesRegex(ValueError, '0 root world rows'):
            park_start(collections, 'DIST_Missing')
        bare = Collections({'collections': PARKS['collections'][:2] + [
            world('dist_bare', 'default_skateparks', WorldStream='DIST_Bare')]})
        with self.assertRaisesRegex(ValueError, 'no start locator'):
            park_start(bare, 'DIST_Bare')

    def test_menu_start_takes_the_chosen_or_only_location(self):
        binary = b'\0A_Only\0B_Left\0B_Right\0'
        rows = [world('default'), world('dist_solo', 'default', WorldStream='DIST_Solo'),
                world('dist_university', 'default', WorldStream='DIST_University'),
                world('dist_many', 'default', WorldStream='DIST_Many'),
                menu('solo', 'dist_solo', 1), menu('solo_alias', 'dist_solo', 1),
                menu('gamestart', 'dist_university', 8), menu('campus', 'dist_university', 15),
                menu('left', 'dist_many', 8), menu('right', 'dist_many', 15)]
        collections = Collections({'collections': rows})
        self.assertEqual(menu_start(collections, binary, 'DIST_Solo'), ('A_Only', 'fe_locations solo, solo_alias'))
        self.assertEqual(menu_start(collections, binary, 'DIST_University'), ('B_Left', 'fe_locations gamestart'))
        with self.assertRaisesRegex(ValueError, '2 fe_locations starts'):
            menu_start(collections, binary, 'DIST_Many')
        with self.assertRaisesRegex(ValueError, 'dist_industrial_reclaimed is not'):
            menu_start(collections, binary, 'DIST_Industrial')

    def test_start_reads_only_the_districts_locator_records(self):
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            park = root/'data/content/global_locators/PARK/DIST_Park'
            other = root/'data/content/global_locators/PARK/DIST_Other'
            park.mkdir(parents=True)
            other.mkdir(parents=True)
            authored = placed((5, 1, -3), (1, 0, 0))
            (park/'a.rx2').write_bytes(locators(('Z_Start', authored), ('elsewhere', placed((0, 0, 0), (0, 0, 1)))))
            (park/'b.rx2').write_bytes(locators(('Z_Start', authored)))  # identical duplicate
            (other/'c.rx2').write_bytes(locators(('Z_Start', placed((9, 9, 9), (0, 0, -1)))))
            found = start(root, PARKS, 'DIST_Park')
            self.assertEqual(found['locator'], 'Z_Start')
            self.assertEqual(found['source'], 'world dist_park')
            # Section at 0x58, records after its 32-byte header.
            self.assertEqual(found['record'], 'data/content/global_locators/PARK/DIST_Park/a.rx2+0x78')
            self.assertEqual(found['position'], [5., 1., -3.])
            self.assertAlmostEqual(found['heading'], math.pi/2)
            (park/'d.rx2').write_bytes(locators(('Z_Start', placed((5, 1, -3), (0, 0, 1)))))
            with self.assertRaisesRegex(ValueError, 'Z_Start has 2 distinct placements'):
                start(root, PARKS, 'DIST_Park')
            missing = {'collections': PARKS['collections'] + [
                world('dist_gone', 'default_skateparks', WorldStream='DIST_Gone', **{START: 'Z_Gone'})]}
            with self.assertRaisesRegex(ValueError, 'Z_Gone has 0 distinct placements'):
                start(root, missing, 'DIST_Gone')

    def test_start_takes_the_records_first_spawn_slot(self):
        # 828E9688 places player slot 0 at a record's first spawn slot when it has any.
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            park = root/'data/content/global_locators/PARK/DIST_Park'
            park.mkdir(parents=True)
            behind = placed((4, 1, -2), (0, 0, -1))
            (park/'a.rx2').write_bytes(locators(
                ('other', placed((0, 0, 0), (0, 0, 1)), ('other::spawn_01', placed((7, 0, 7), (0, 0, 1)))),
                ('Z_Start', placed((5, 1, -3), (1, 0, 0)), ('Z_Start::spawn_01', behind),
                 ('Z_Start::spawn_02', placed((6, 1, -2), (0, 0, -1))))))
            (park/'b.rx2').write_bytes(locators(('Z_Start', placed((5, 1, -3), (1, 0, 0)), ('Z_Start::spawn_01', behind))))
            found = start(root, PARKS, 'DIST_Park')
            self.assertEqual(found['locator'], 'Z_Start::spawn_01')
            # Records at 0x78; slots follow both records, after the other record's one slot.
            self.assertEqual(found['record'], 'data/content/global_locators/PARK/DIST_Park/a.rx2+0x1e8')
            self.assertEqual(found['position'], [4., 1., -2.])
            self.assertAlmostEqual(found['heading'], math.pi)

    def test_chosen_starts_are_limited_to_cities_and_maloof(self):
        self.assertEqual(set(DEFAULT_LOCATIONS),
                         {'DIST_University', 'DIST_Industrial', 'DIST_DownTown', 'DIST_MaloofMoneyCup'})


if __name__ == '__main__':
    unittest.main()
