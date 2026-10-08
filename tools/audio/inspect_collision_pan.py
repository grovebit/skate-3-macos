"""Audit authored AEMS pan choices in an owned SPLC v3 collision bank.

Does not infer selection modes, layer lifetimes or Pn21 template parameters.
"""
import argparse
from collections import Counter
from dataclasses import asdict
import hashlib
import json
from pathlib import Path
import math

from tools.owned_game.splc import event_pan_choices


def inspect(data: bytes, event: int | None = None) -> dict:
    events = event_pan_choices(data)
    choices = {choice.offset: choice for variants in events for variant in variants
               for layer in variant for choice in layer}
    if any(not math.isfinite(c.angle_degrees) or not math.isfinite(c.pitch_multiplier)
           for c in choices.values()):
        raise ValueError('Nonfinite authored pan/pitch field is outside the verified domain')
    result = {'sha256': hashlib.sha256(data).hexdigest(), 'events': len(events),
              'choices': len(choices),
              'angle_counts': dict(sorted(Counter(c.angle_degrees for c in choices.values()).items())),
              'pitch_multipliers': sorted({c.pitch_multiplier for c in choices.values()})}
    if event is not None:
        if not 0 <= event < len(events):
            raise ValueError('Event index outside bank')
        result['event'] = event
        result['variants'] = [[[asdict(c) for c in layer] for layer in variant]
                              for variant in events[event]]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bank', type=Path)
    parser.add_argument('--event', type=int)
    args = parser.parse_args()
    try:
        if args.bank.stat().st_size > 256 * 1024 * 1024:
            raise ValueError('Bank exceeds the 256 MiB inspection limit')
        print(json.dumps(inspect(args.bank.read_bytes(), args.event), indent=2))
    except (ValueError, OSError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
