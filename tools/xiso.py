"""Extract an Xbox or Xbox 360 disc image (XDVDFS, "XISO") with no extra tools.

    python3 -m tools.xiso "Skate 3.iso" "/path/to/Skate 3"

The volume descriptor sits 32 sectors into the game partition. Full Xbox 360
dumps place that partition after the video partition (XGD2 0xFD90000, XGD3
0x2080000); trimmed images start with it, and original Xbox dumps (XGD1) use
0x18300000. A directory table is a binary tree of entries: u16 left and right
child offsets in 4-byte units, u32 start sector, u32 size, u8 attributes, u8
name length and the name, each entry 4-byte aligned and padded with 0xFF.
"""
import argparse
import os
from pathlib import Path
import shutil
import struct

SECTOR = 2048
MAGIC = b'MICROSOFT*XBOX*MEDIA'
PARTITIONS = (0, 0x0FD90000, 0x02080000, 0x18300000)
DIRECTORY = 0x10
PAD = 0xFFFF
ENTRY = struct.Struct('<HHIIBB')
CHUNK = 8 << 20


class XisoError(RuntimeError):
    pass


def volume(image):
    """Return (partition offset, root table sector, root table size)."""
    for offset in PARTITIONS:
        image.seek(offset + 32 * SECTOR)
        header = image.read(SECTOR)
        if len(header) == SECTOR and header.startswith(MAGIC) and header.endswith(MAGIC):
            return (offset, *struct.unpack_from('<II', header, len(MAGIC)))
    raise XisoError('Not an Xbox disc image: no XDVDFS volume descriptor found')


def entries(table):
    """Yield (name, sector, size, attributes) for one directory table."""
    if not table or table[:2] == b'\xff\xff':
        return  # empty directory
    pending, seen = [0], set()
    while pending:
        offset = pending.pop()
        if offset in seen:
            raise XisoError('Directory tree refers back to itself')
        seen.add(offset)
        if offset + ENTRY.size <= len(table) and table[offset:offset+2] == b'\xff\xff':
            # A child offset into sector padding means the entry that starts
            # the next sector, as extract-xiso reads such tables.
            offset = (offset // SECTOR + 1) * SECTOR
            seen.add(offset)
        if offset + ENTRY.size > len(table):
            raise XisoError('Directory entry runs past its table')
        left, right, sector, size, attributes, length = ENTRY.unpack_from(table, offset)
        start = offset + ENTRY.size
        if left == PAD or start + length > len(table):
            raise XisoError('Corrupt directory entry')
        name = table[start:start+length].decode('latin-1')
        if not name or name in {'.', '..'} or any(c in name for c in '/\\\0'):
            raise XisoError(f'Unsafe file name in disc image: {name!r}')
        yield name, sector, size, attributes
        pending.extend(4 * child for child in (right, left) if child)


def walk(image):
    """Yield (relative path, absolute offset or None for directories, size)."""
    partition, sector, size = volume(image)
    end = image.seek(0, os.SEEK_END)
    stack, tables = [(Path(), sector, size)], set()
    while stack:
        parent, sector, size = stack.pop()
        if (sector, size) in tables:
            raise XisoError('Directory tree refers back to itself')
        tables.add((sector, size))
        offset = partition + sector * SECTOR
        if offset + size > end:
            raise XisoError(f'Directory {parent.as_posix() or "/"} runs past the end of the image')
        image.seek(offset)
        names = set()
        for name, start, length, attributes in entries(image.read(size)):
            if name.casefold() in names:
                raise XisoError(f'Duplicate name in disc image: {(parent/name).as_posix()}')
            names.add(name.casefold())
            if attributes & DIRECTORY:
                yield parent/name, None, 0
                stack.append((parent/name, start, length))
            else:
                if partition + start * SECTOR + length > end:
                    raise XisoError(f'{(parent/name).as_posix()} runs past the end of the image')
                yield parent/name, partition + start * SECTOR, length


def extract(iso, destination, report=print):
    """Extract every file of the image into a new destination directory."""
    destination = Path(destination)
    with open(iso, 'rb') as image:
        items = list(walk(image))
        needed = sum(size for _, offset, size in items if offset is not None)
        destination.mkdir(parents=True)
        free = shutil.disk_usage(destination).free
        if needed > free:
            raise XisoError(f'Extracting needs {needed / 1e9:.1f} GB free; {free / 1e9:.1f} GB available')
        done, shown = 0, -1
        for path, offset, size in items:
            target = destination/path
            if offset is None:
                target.mkdir()
                continue
            image.seek(offset)
            with open(target, 'wb') as output:
                remaining = size
                while remaining:
                    data = image.read(min(CHUNK, remaining))
                    if not data:
                        raise XisoError(f'{path.as_posix()} is truncated')
                    output.write(data)
                    remaining -= len(data)
                    done += len(data)
                    percent = 100 * done // max(needed, 1)
                    if percent // 10 != shown:
                        shown = percent // 10
                        report(f'Extracting disc image: {percent}%')
    return destination


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('iso', type=Path)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args(argv)
    try:
        extract(args.iso, args.destination, lambda text: print(text, flush=True))
    except (OSError, XisoError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
