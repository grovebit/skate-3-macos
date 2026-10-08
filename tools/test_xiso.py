import struct
import tempfile
import unittest
from pathlib import Path

from tools import xiso

SECTOR = xiso.SECTOR


def table(items):
    """Directory table for sorted (name, sector, size, attributes) items.

    Entries are stored in pre-order of a balanced tree, so the root is at
    offset 0, and padded to the next sector when one would cross a boundary.
    """
    order, children = [], {}

    def build(lo, hi):
        if lo >= hi:
            return None
        mid = (lo + hi) // 2
        order.append(mid)
        children[mid] = (build(lo, mid), build(mid + 1, hi))
        return mid

    build(0, len(items))
    data, offsets = bytearray(), {}
    for index in order:
        size = (xiso.ENTRY.size + len(items[index][0]) + 3) & ~3
        if len(data) % SECTOR + size > SECTOR:
            data += b'\xff' * (SECTOR - len(data) % SECTOR)
        offsets[index] = len(data)
        data += bytes(size)
    for index in order:
        name, sector, size, attributes = items[index]
        left, right = (offsets[c] // 4 if c is not None else 0 for c in children[index])
        entry = xiso.ENTRY.pack(left, right, sector, size, attributes, len(name)) + name.encode()
        end = offsets[index] + len(entry)
        data[offsets[index]:end] = entry
        data[end:(end + 3) & ~3] = b'\xff' * (((end + 3) & ~3) - end)
    data += b'\xff' * (-len(data) % SECTOR)
    return bytes(data)


class Image:
    """Writes an XDVDFS image: tree maps names to bytes or nested dicts."""

    def __init__(self, path, partition=0):
        self.path, self.partition, self.next = path, partition, 33
        self.writes = []

    def allocate(self, data):
        sector = self.next
        self.next += max(1, -(-len(data) // SECTOR))
        self.writes.append((sector, data))
        return sector

    def directory(self, tree):
        items = []
        for name in sorted(tree, key=str.upper):
            value = tree[name]
            if isinstance(value, dict):
                data = self.directory(value)
                items.append((name, self.allocate(data) if data else 0, len(data), xiso.DIRECTORY))
            else:
                items.append((name, self.allocate(value) if value else 0, len(value), 0x80))
        return table(items) if items else b''

    def write(self, tree, root=None):
        data = root if root is not None else self.directory(tree)
        sector = self.allocate(data)
        header = xiso.MAGIC + struct.pack('<IIQ', sector, len(data), 0)
        header += bytes(SECTOR - len(header) - len(xiso.MAGIC)) + xiso.MAGIC
        with open(self.path, 'wb') as image:
            for at, data in [(32, header)] + self.writes:
                image.seek(self.partition + at * SECTOR)
                image.write(data)
        return self.path


def files(root):
    return {p.relative_to(root).as_posix(): (p.read_bytes() if p.is_file() else None)
            for p in sorted(root.rglob('*'))}


class XisoTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def test_extracts_trees_at_every_partition_offset(self):
        content = {f'worldDIST_{i:03}.big': bytes([i]) * (i * 97) for i in range(120)}
        tree = {'default.xex': b'XEX2' + bytes(5000), 'empty.bin': b'',
                'data': {'content': content, 'big': {'db.big': b'db' * 3000}, 'none': {}},
                'nxeart': b'art'}
        expected = {'data': None, 'data/big': None, 'data/big/db.big': b'db' * 3000,
                    'data/content': None, 'data/none': None, 'default.xex': b'XEX2' + bytes(5000),
                    'empty.bin': b'', 'nxeart': b'art'}
        expected.update({f'data/content/{n}': v for n, v in content.items()})
        for partition in xiso.PARTITIONS:
            with self.subTest(partition=hex(partition)):
                iso = Image(self.root / f'{partition}.iso', partition).write(tree)
                out = xiso.extract(iso, self.root / f'out-{partition}', report=lambda _: None)
                self.assertEqual(files(out), expected)

    def test_child_offset_into_padding_reads_next_sector(self):
        long = 'b' * 200
        # The 15-byte root's right child (offset 4, 16 bytes) points into the
        # padding in front of the entry that starts the second sector.
        root = xiso.ENTRY.pack(0, 4, 0, 0, 0x80, 1) + b'a'
        data = bytearray(root + b'\xff' * (SECTOR - len(root)))
        data += xiso.ENTRY.pack(0, 0, 0, 0, 0x80, len(long)) + long.encode()
        data += b'\xff' * (-len(data) % SECTOR)
        iso = Image(self.root / 'pad.iso').write({}, root=bytes(data))
        out = xiso.extract(iso, self.root / 'out', report=lambda _: None)
        self.assertEqual(set(files(out)), {'a', long})

    def test_rejects_images_that_are_not_xbox_discs(self):
        (self.root / 'plain.iso').write_bytes(bytes(40 * SECTOR))
        with self.assertRaisesRegex(xiso.XisoError, 'Not an Xbox disc image'):
            xiso.extract(self.root / 'plain.iso', self.root / 'out')

    def test_rejects_unsafe_names_without_writing(self):
        for name in ('..', 'a/b', 'a\\b'):
            with self.subTest(name=name):
                iso = Image(self.root / 'bad.iso').write({}, root=table([(name, 0, 0, 0x80)]))
                with self.assertRaisesRegex(xiso.XisoError, 'Unsafe file name'):
                    xiso.extract(iso, self.root / 'out')
                self.assertFalse((self.root / 'out').exists())

    def test_rejects_case_insensitive_duplicates(self):
        iso = Image(self.root / 'dup.iso').write({}, root=table([('A', 0, 0, 0x80), ('a', 0, 0, 0x80)]))
        with self.assertRaisesRegex(xiso.XisoError, 'Duplicate name'):
            xiso.extract(iso, self.root / 'out')

    def test_rejects_loops(self):
        # 15-byte entries pad to 16, so offset 4 (16 bytes) is "b", whose
        # right child is itself.
        entry = xiso.ENTRY.pack(0, 4, 0, 0, 0x80, 1) + b'a' + b'\xff'
        entry += xiso.ENTRY.pack(0, 4, 0, 0, 0x80, 1) + b'b' + b'\xff'
        looped = entry + b'\xff' * (SECTOR - len(entry))
        iso = Image(self.root / 'loop.iso').write({}, root=looped)
        with self.assertRaisesRegex(xiso.XisoError, 'refers back to itself'):
            xiso.extract(iso, self.root / 'out')
        # A subdirectory whose table is its own parent's.
        image = Image(self.root / 'nested.iso')
        sector = image.next
        root = table([('d', sector, SECTOR, xiso.DIRECTORY)])
        with self.assertRaisesRegex(xiso.XisoError, 'refers back to itself'):
            xiso.extract(image.write({}, root=root), self.root / 'out2')

    def test_rejects_files_past_the_end(self):
        iso = Image(self.root / 'short.iso').write({}, root=table([('big', 40, 10 * SECTOR, 0x80)]))
        with self.assertRaisesRegex(xiso.XisoError, 'past the end'):
            xiso.extract(iso, self.root / 'out')


if __name__ == '__main__':
    unittest.main()
