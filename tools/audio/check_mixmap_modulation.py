"""Evaluate an authored MixMap node's levels from explicit distance and phase.

This is an offline level checker. It does not publish audio, select a live mode,
reproduce phase generation, or evaluate the node's auxiliary pitch output.
"""
import argparse
import hashlib
import json
from pathlib import Path

from tools.owned_game.mixmap import modulation_routes
from tools.owned_game.mixmap_curve import read_curve_table
from tools.owned_game.mixmap_modulation import evaluate_node, read_log_table


def _read(path, limit):
    with path.open('rb') as source:
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f'{path.name} exceeds the {limit}-byte inspection limit')
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mixmap', type=Path, required=True)
    parser.add_argument('--curve-table', type=Path, required=True)
    parser.add_argument('--log-table', type=Path, required=True)
    parser.add_argument('--group', type=int, required=True)
    parser.add_argument('--node', type=int, required=True)
    parser.add_argument('--mode-index', type=int, required=True,
                        help='Authored block index; no live mode selection is inferred')
    parser.add_argument('--distance', type=float, required=True,
                        help='Selected float value after the mode input selector')
    parser.add_argument('--phase', type=int, required=True,
                        help='Selected packed phase after the mode input selector')
    parser.add_argument('--disabled', action='store_true')
    args = parser.parse_args()
    try:
        if min(args.group, args.node, args.mode_index) < 0:
            raise ValueError('Group, node and mode indices must be nonnegative')
        program = _read(args.mixmap, 1024 * 1024)
        curve_data = _read(args.curve_table, 513 * 4)
        log_data = _read(args.log_table, 512 * 4)
        node = modulation_routes(program)[args.group]['nodes'][args.node]
        mode = node['modes'][args.mode_index]
        if mode['float_input_slot'] is None and args.distance != -1.0:
            raise ValueError('This mode selects constant distance -1.0')
        if mode['phase_input_slot'] is None and args.phase != 0:
            raise ValueError('This mode selects constant phase zero')
        result = evaluate_node(args.distance, args.phase, mode['bounds'],
                               int(mode['curve_word'], 16), read_curve_table(curve_data),
                               read_log_table(log_data), enabled=not args.disabled)
        result.update(controller_id=node['controller_id'], mode_word=mode['mode_word'],
                      mixmap_sha256=hashlib.sha256(program).hexdigest(),
                      curve_table_sha256=hashlib.sha256(curve_data).hexdigest(),
                      log_table_sha256=hashlib.sha256(log_data).hexdigest())
    except IndexError:
        parser.error('Requested group, node or mode block does not exist')
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
