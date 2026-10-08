"""Check enabled MixMap format-0 gain using explicit levels and an owned table.

Accepts a 2408-byte table extracted at mapped base-disc address 82FBB430.
This evaluates only the final conversion, not the live control expressions.
"""
import argparse
import hashlib
import json
from pathlib import Path

from tools.owned_game.mixmap_volume import format0_volume, read_volume_table


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    number = lambda text: int(text, 0)
    parser.add_argument('--table', type=Path, required=True)
    parser.add_argument('--record-level', type=number, required=True,
                        help='Explicit evaluated output-record level')
    parser.add_argument('--adjustment', type=number, required=True,
                        help='Signed low-16 field of the authored write word')
    parser.add_argument('--modulation-level', type=number,
                        help='Explicit modulation-node level; omit for the direct path')
    args = parser.parse_args()
    try:
        with args.table.open('rb') as source:
            data = source.read(602 * 4 + 1)
        result = format0_volume(args.record_level, args.adjustment,
                               read_volume_table(data), args.modulation_level)
        result['table_sha256'] = hashlib.sha256(data).hexdigest()
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
