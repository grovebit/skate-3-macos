"""Audit every authored control referenced by an owned collisions.json export."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from tools.owned_game.splc import event_controls


MODES = {0: 'random', 1: 'sequential', 2: 'alternating_half_shuffle'}


def counts(values):
    return dict(sorted(Counter(map(str, values)).items()))


def inspect(data: bytes, event_ids) -> dict:
    events = event_controls(data)
    selected = []
    for event in sorted(set(map(int, event_ids))):
        if not 0 <= event < len(events):
            raise ValueError('Event index outside bank')
        selected.append(events[event])
    leaves = {v.offset: v for e in selected for v in e.variants}
    layers = {l.offset: l for v in leaves.values() for l in v.layers}
    choices = {c.offset: c for l in layers.values() for c in l.choices}
    return {
        'sha256': hashlib.sha256(data).hexdigest(),
        'events': len(selected),
        'event_modes': counts('leaf' if e.selection_mode is None else MODES[e.selection_mode]
                              for e in selected),
        'unique_leaves': len(leaves), 'unique_layers': len(layers),
        'unique_choices': len(choices),
        'leaf_layer_counts': counts(len(v.layers) for v in leaves.values()),
        'layer_modes': counts(MODES[l.selection_mode] for l in layers.values()),
        'layer_choice_counts': counts(len(l.choices) for l in layers.values()),
        'leaf_fields': {field: counts(getattr(v, field) for v in leaves.values())
                        for field in ('gain', 'source_rate', 'source_rate_range')},
        'choice_fields': {field: counts(getattr(c, field) for c in choices.values())
                          for field in CHOICE_FIELDS},
        'summary': {
            'positive_delay': sum(c.delay > 0 for c in choices.values()),
            'nonzero_delay_range': sum(c.delay_range != 0 for c in choices.values()),
            'nonunity_gain': sum(c.gain != 1 for c in choices.values()),
            'nonunity_gain_randomization': sum(c.gain_randomization != 1 for c in choices.values()),
            'attack': sum(c.attack_end != 0 for c in choices.values()),
            'release': sum(c.release_start != 0 for c in choices.values()),
        },
    }


CHOICE_FIELDS = ('gain', 'source_rate', 'pitch_multiplier', 'angle_degrees', 'delay',
                 'source_start', 'source_end', 'attack_end', 'release_start',
                 'envelope_curve', 'gain_randomization', 'source_rate_range',
                 'delay_range', 'probability', 'routing_flag', 'effect_index', 'priority')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--bank-dir', type=Path, action='append', required=True)
    args = parser.parse_args()
    raw = args.manifest.read_bytes()
    manifest = json.loads(raw)
    report = {'manifest_sha256': hashlib.sha256(raw).hexdigest(), 'banks': {}}
    for bank, events in manifest['events'].items():
        paths = [d / (bank + '.bnk') for d in args.bank_dir if (d / (bank + '.bnk')).is_file()]
        if len(paths) != 1:
            parser.error(f'Expected exactly one owned {bank}.bnk')
        report['banks'][bank] = inspect(paths[0].read_bytes(), events)
    print(json.dumps(report, indent=2, allow_nan=False))


if __name__ == '__main__':
    main()
