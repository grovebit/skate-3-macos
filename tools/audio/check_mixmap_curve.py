"""Evaluate original MixMap curves using a table extracted from an owned game.

The 2052-byte table starts at mapped base-disc address 82FBC598. Inputs are
explicit normalized integers or floats, not inferred live controls.
"""
import argparse
import hashlib
import json
from pathlib import Path

from tools.owned_game.mixmap_curve import evaluate_curve, evaluate_float_curve, read_curve_table


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--table', type=Path, required=True)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument('--input', type=int)
    inputs.add_argument('--float-input', type=float)
    parser.add_argument('--curve', type=int, required=True)
    args = parser.parse_args()
    try:
        with args.table.open('rb') as source:
            data = source.read(513 * 4 + 1)
        table = read_curve_table(data)
        if args.float_input is None:
            result = evaluate_curve(args.input, args.curve, table)
        else:
            result = evaluate_float_curve(args.float_input, args.curve, table)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps({'level': result, 'table_sha256': hashlib.sha256(data).hexdigest()},
                     indent=2))


if __name__ == '__main__':
    main()
