"""Inspect original audio MixMap declarations without executing the control program."""
import argparse
import json
from pathlib import Path

from tools.owned_game.mixmap import declarations, modulation_routes, output_routes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mixmap', type=Path)
    args = parser.parse_args()
    try:
        # Diagnostic input bound; the owned base-disc file is 25252 bytes.
        with args.mixmap.open('rb') as source:
            data = source.read(1024 * 1024 + 1)
        if len(data) > 1024 * 1024:
            raise ValueError('MixMap exceeds the 1 MiB inspection limit')
        result = declarations(data)
        result['output_groups'] = output_routes(data)
        result['modulation_groups'] = modulation_routes(data)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
