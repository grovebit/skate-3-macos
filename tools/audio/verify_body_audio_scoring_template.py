"""Replay the owned base-disc Scoring1 template copy, including duration.

Requires private mapped default.pe and ppc_interp.py; stores no executable bytes.
This verifies packet copying, not worker lifecycle or global template writers.
"""
import argparse
import hashlib
from pathlib import Path
import random
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--analysis-dir', type=Path, required=True)
    args = parser.parse_args()
    expected = 'ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42'
    if hashlib.sha256((args.analysis_dir / 'default.pe').read_bytes()).hexdigest() != expected:
        parser.error('default.pe does not match the verified base-disc mapped image')
    sys.path.insert(0, str(args.analysis_dir.resolve()))
    from ppc_interp import Machine

    rng = random.Random(0x82DBA980)
    for index in range(100):
        m = Machine()
        m.writable = [(0x100000, 0x130000), (0x207000, 0x209000)]
        m.r[1], m.r[30], m.r[31] = 0x208000, 0x100000, 0x101000
        m.w32(0x10003C, 0x110000)
        m.w32(0x10103C, 0x120000)
        template = bytearray(rng.randbytes(0x3950))
        # Include zero default duration and arbitrary bit patterns: copying is raw.
        if index % 2 == 0:
            template[0x3518:0x351C] = bytes(4)
        m.write(0x110000, bytes(template))
        m.write(0x120000, rng.randbytes(0x3950))
        m.call(0x82DBA980, sentinel=0x82DBA990)
        if m.read(0x120000, 0x3950) != bytes(template):
            raise RuntimeError(f'Scoring1 template copy mismatch at fixture {index}')
    print('100 native Scoring1 template copies matched all 0x3950 bytes, including duration')


if __name__ == '__main__':
    main()
