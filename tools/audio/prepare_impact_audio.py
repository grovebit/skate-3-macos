"""Prepare material-specific impact samples from an owned Skate 3 installation.

Representative routes support existing playback; primary-body event matrices
preserve recovered category/counterpart selection for the ongoing port. Sample
choices retain layers, mixed offline with provisional host gain. Original
envelopes, pitch and randomization modes are not reconstructed.

collisions.json carries every material ID's original bank, event matrix and
voice controls (82FD1930, 82484410, 82484638), plus the authored body-loop
settings, with one rendered route per referenced bank event.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tempfile

from tools.asset_pipeline.setup_state import source_directory
from tools.asset_pipeline.vlt import convert
from tools.owned_game.big import BigArchive
from tools.owned_game.splc import event_layers, streams
from tools.owned_game.impact_mix import render
from tools.owned_game.impact_intensity import intensity_tables
from tools.owned_game.impact_volume import volume_tables
from tools.owned_game.body_events import body_event_routes
from tools.owned_game.body_controls import body_controls
from tools.owned_game.audio_surface import surface_classes
from tools.owned_game.collision_materials import BANKS, body_settings, collision_materials
from tools.owned_game import collision_mix
from tools.audio.prepare_skating_audio import decoder_info, validate_wav

# Collision-tag audio surface enum, also used by the owned-world material editor.
SURFACES = '''undefined asphalt_smooth asphalt_rough concrete_polished concrete_rough
concrete_aggregate wood_ramp plywood dirt metal grass metal_solid_round_1
metal_solid_round_1_up metal_solid_round_2 metal_solid_square_1 metal_solid_square_2
metal_hollow_round_1 metal_hollow_round_1_dead metal_hollow_round_1_dn
metal_hollow_round_2 metal_hollow_round_2_dead metal_hollow_round_2_dn
metal_hollow_round_3 metal_hollow_round_4 metal_hollow_square_1 metal_hollow_square_2
metal_hollow_square_3 metal_hollow_square_3_dead metal_hollow_square_4 metal_hollow_1
metal_hollow_2 metal_sheet metal_complex_1 metal_complex_2 metal_complex_3
metal_complex_4 metal_complex_5 metal_complex_6 metal_complex_7 metal_complex_8
metal_complex_debris wood_1 wood_1_up wood_2 wood_3 wood_3_up wood_4 plastic_1
plastic_2 plastic_3 plastic_4 glass_thick_large glass_thin_small concrete_curb
concrete_bench leaves bush pottery paper cardboard garbage_bag garbage_spill bottle
 tile_ceramic marble_or_slate brick_smooth brick_course manhole_metal
metal_grate_sewer metal_grate_planter deepsnow packedsnow ice antennas chandelier
plexiglass_small plexiglass_large potted_plant crumpled_paper cloth pop_can paper_cup
wire_cable volleyball oil_drum dmo_rail fruit plastic_bottle drum_pylon metal_rail_4
wood_5 metal_ramp complex_plastic_1'''.split()

# Representative routes retained for provisional host playback. The recovered
# primary-body event matrix is exported separately by body_event_routes.
EVENT_FIELDS = [('Skate_Collisions', 'Hash_BFABF634D2B1E45A'),
                ('Skate_Metal', 'Hash_595537EBA0196BE7')]


def select_material(row: dict, events: dict) -> dict | None:
    for bank, field in EVENT_FIELDS:
        value = row['fields'].get(field)
        if not value or value['type'] != bank:
            continue
        event = int(value['data'], 16)
        if not event:
            continue
        return select_event(bank, event, field, row["key"], events)
    return None


def select_event(bank: str, event: int, field: str, material: str, events: dict) -> dict:
    if not 0 <= event < len(events[bank]):
        raise ValueError(f"{material}: event outside {bank}")
    variants = events[bank][event]
    if not variants or any(not variant or len(variant) > 8
                           or any(not layer for layer in variant) for variant in variants):
        raise ValueError(f'{material}: unsupported impact layers')
    # Keep several leaf variants; a single leaf with multiple choices also
    # gets alternatives. One choice per layer sounds in each rendered clip.
    count = min(4, max(len(variants), max(len(layer) for v in variants for layer in v)))
    selected = [[f'{bank}/{layer[(i // len(variants)) % len(layer)]:04d}.wav'
                 for layer in variants[i % len(variants)]] for i in range(count)]
    return {'material': material, 'bank': bank, 'event': event, 'field': field,
            'source_layers': selected,
            'clips': [f'{bank}/events/{event:04d}-{i}.wav' for i in range(count)]}


def owned_collections(game: Path, work: Path) -> tuple[dict, list[dict]]:
    """Convert the owned VLT database from db.big into `work`."""
    database = BigArchive(game / 'data/big/db.big')
    needed = {'skaterschema.bin', 'skaterschema.vlt', 'skatercollections.bin',
              'skatercollections.vlt', 'skaterschema_summaryreport.txt'}
    provenance = []
    entries = [e for e in database.entries if e.path.startswith('data/db/')
               and Path(e.path).name in needed]
    if (len(entries) != len(needed)
            or {Path(e.path).name for e in entries} != needed):
        raise ValueError('Missing or ambiguous owned audio database inputs')
    for entry in entries:
        data = database.read(entry)
        (work / Path(entry.path).name).write_bytes(data)
        provenance.append({'entry': entry.path, 'sha256': hashlib.sha256(data).hexdigest()})
    names = []
    for line in (work / 'skaterschema_summaryreport.txt').read_text().splitlines():
        names.extend(line.replace('.class', '').replace('.xml', '').split('/'))
    return convert(work / 'skaterschema', work / 'skatercollections', names), provenance


def prepare(game: Path, output: Path, decoder: str) -> dict:
    game = source_directory(game, require_core=False)
    output = output.resolve()
    if output.exists():
        raise FileExistsError(f'Refusing to overwrite {output}; choose another --output')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.impact-audio-', dir=output.parent) as temp:
        work = Path(temp)
        staging = work / 'library'
        staging.mkdir()
        collections, provenance = owned_collections(game, work)
        # Verified before rendering: the runtime needs mixmap/ for collisions.json.
        mix_inputs = collision_mix.inputs(game, collections['collections'])
        material_intensity = intensity_tables(collections['collections'])
        material_volume = volume_tables(collections['collections'])
        primary_body_controls = body_controls(collections['collections'])
        counterpart_classes = surface_classes(collections['collections'])
        collision = collision_materials(collections['collections'])
        settings = body_settings(collections['collections'])
        for material in collision:
            if material is None:
                continue
            intensity = material_intensity['tables'].get(material_intensity['materials'][material['name']])
            volume = material_volume['tables'].get(material_volume['materials'][material['name']] or '')
            if intensity is None or volume is None:
                raise ValueError(f"{material['name']}: missing collision material profile")
            material['intensity'] = intensity
            material['volume'] = volume
        materials = {r['key']: r for r in collections['collections'] if r['class'] == 'aud_material'}
        archive = BigArchive(game / 'data/audio/audiofiles.big')
        banks = {}
        events = {}
        for bank in BANKS:
            entry = next(e for e in archive.entries if e.path == f'data/audio/{bank}.bnk')
            data = archive.read(entry)
            banks[bank] = (data, streams(data))
            events[bank] = event_layers(data)
            provenance.append({'entry': entry.path, 'sha256': hashlib.sha256(data).hexdigest()})
        routes = []
        for surface, name in enumerate(SURFACES):
            if name in materials and (route := select_material(materials[name], events)):
                routes.append({'surface': surface, **route})
        body_parts = {part: select_material(materials[part], events)
                      for part in ['head', 'torso', 'arm', 'leg', 'foot']}
        body = body_parts['torso']
        if not routes or any(route is None for route in body_parts.values()):
            raise ValueError('Owned database has no supported impact routes')
        from tools.owned_game.contact_audio import contact_routes
        wheel_contacts = contact_routes(collections["collections"], events, select_event)
        body_events = body_event_routes(collections['collections'], events, select_event)
        collision_events = {}
        for material in collision:
            if material is None:
                continue
            bank = BANKS[material['bank']]
            matrix = material['events']
            for event in [*matrix['soft'], *matrix['medium'], matrix['hard']]:
                if (bank, event) not in collision_events:
                    collision_events[bank, event] = select_event(
                        bank, event, 'collision', material['name'], events)
        all_routes = [*routes, *body_parts.values(),
                      *(route for categories in body_events.values()
                        for category in categories for route in category),
                      *(route for mode in wheel_contacts["modes"] for route in mode if route),
                      *collision_events.values()]
        clips = sorted({clip for route in all_routes
                        for variant in route['source_layers'] for clip in variant})
        decoded = []
        for clip in clips:
            path = Path(clip)
            data, embedded = banks[path.parent.name]
            stream = embedded[int(path.stem) - 1]
            source = work / 'sample.snr'
            source.write_bytes(data[stream.offset:stream.offset + stream.size])
            target = staging / path
            target.parent.mkdir(exist_ok=True)
            info = decoder_info(decoder, source, output=target)
            decoded.append({'file': clip, 'source_hash': f'{stream.source_hash:08x}',
                            'decoder_version': info['version'], **validate_wav(target, info)})
        jobs = {clip: layers for route in all_routes
                for clip, layers in zip(route['clips'], route['source_layers'])}
        rendered = [{'file': clip, 'source_layers': layers,
                     **render([staging / source for source in layers], staging / clip)}
                    for clip, layers in sorted(jobs.items())]
        trims = {clip['file']: clip['playback_trim'] for clip in rendered}
        for route in all_routes:
            route['gains'] = [trims[clip] for clip in route['clips']]
        # Each manifest lists only its own routes' clips; the runtime bounds
        # routes.json, so collision-only clips must not inflate it.
        collision_routes = list(collision_events.values())
        legacy_routes = all_routes[:len(all_routes) - len(collision_routes)]

        def used(selected):
            outputs = {clip for route in selected for clip in route['clips']}
            sources = {clip for route in selected for variant in route['source_layers'] for clip in variant}
            return ([r for r in rendered if r['file'] in outputs], [d for d in decoded if d['file'] in sources])

        legacy_rendered, legacy_decoded = used(legacy_routes)
        manifest = {'rendered': legacy_rendered, 'version': 1, 'surfaces': routes, 'body': body, 'body_parts': body_parts, 'sources': provenance,
                    'decoded': legacy_decoded, 'wheel_contacts': wheel_contacts,
                    'body_events': body_events, 'body_controls': primary_body_controls,
                    'surface_classes': counterpart_classes,
                    'material_intensity': material_intensity, 'material_volume': material_volume, 'limitations': [
                        'Material tables, event matrices and voice controls support primary-body diagnostics. Playback scheduling, special-mode overrides and live controller integration remain unported.',
                        'Primary body event matrices preserve original category/counterpart selection; runtime playback still uses representative events and provisional gain.',
                        'Layer choices are mixed simultaneously with host headroom; original timing, envelopes, pitch and randomizers are not reproduced.',
                        'Body hits use head, torso, arm, leg or foot samples; surface-dependent body mixing is not implemented.',
                        'Unmapped surfaces use the existing generic impact fallback.']}
        (staging / 'routes.json').write_text(json.dumps(manifest, indent=2) + '\n')
        collision_rendered, collision_decoded = used(collision_routes)
        collisions = {
            'version': 1, 'sources': provenance, 'banks': list(BANKS), 'materials': collision,
            'rendered': collision_rendered, 'decoded': collision_decoded,
            'settings': settings, 'surface_classes': counterpart_classes,
            'events': {bank: {str(event): {key: route[key] for key in ('clips', 'gains', 'source_layers')}
                              for (route_bank, event), route in sorted(collision_events.items())
                              if route_bank == bank} for bank in BANKS},
            'limitations': [
                'Material IDs, banks, event fields, body settings and voice controls are original data.',
                'Each event keeps up to four layer mixes; original randomizers, timing and envelopes are not reproduced.',
                'Special pitch, bank-8 and Hall of Meat set selection overrides are not exported.']}
        (staging / 'collisions.json').write_text(json.dumps(collisions, indent=1) + '\n')
        collision_mix.write(staging / 'mixmap', *mix_inputs, provenance)
        if output.exists():
            raise FileExistsError(f'Output appeared during preparation: {output}')
        staging.rename(output)
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('--output', type=Path, default=Path('.local/skating-audio/material-impacts'))
    parser.add_argument('--decoder', default='vgmstream-cli')
    args = parser.parse_args()
    decoder = shutil.which(args.decoder)
    if decoder is None:
        parser.exit(1, 'vgmstream-cli is required\n')
    try:
        manifest = prepare(args.game, args.output, decoder)
    except (ValueError, OSError, RuntimeError) as error:
        parser.exit(1, f'{error}\n')
    print(f'Prepared {len(manifest["surfaces"])} material routes and {len(manifest["rendered"])} impact variants from {len(manifest["decoded"])} original clips')


if __name__ == '__main__':
    main()
