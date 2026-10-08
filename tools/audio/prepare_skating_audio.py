"""Decode owned skating sound banks into a local audition library.

Run: python3 -m tools.audio.prepare_skating_audio '/path/to/Skate 3'
Requires vgmstream-cli (on macOS: brew install vgmstream).
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import html
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import wave

from tools.owned_game.big import BigArchive
from tools.owned_game.splc import streams as splc_streams
from tools.asset_pipeline.setup_state import source_directory


# ABK banks decode directly; the collision bank embeds SNR streams in SPLC.
BANKS = (
    'GRINDS.abk', 'Rolling_Rattles.abk', 'Sk8_Air_Flip_Tricks.abk',
    'fstep_skateshoe1_sm.abk', 'WHEEL_SKID_BANK.abk', 'FOOT_DRAG.abk',
    'board_scrapes.abk', 'Skate_Collisions.bnk',
)
LIMITATIONS = [
    'Collision samples are decoded; their authored event/control graphs are not reconstructed.',
    'Skate_Metal.bnk and sk8_foley.bnk are not included in this library.',
    'Surface-dependent granular wheel rolling in grains.big is not decoded.',
    'Bank/subsong IDs are preserved; original event and surface mappings are not yet reconstructed.',
    'The initial gameplay mix uses representative clips from this library and provisional footstep timing.',
]


def decoder_info(decoder: str, source: Path, index: int = 1, output: Path | None = None) -> dict:
    command = [decoder, '-I', '-i', '-s', str(index)]
    command += ['-m'] if output is None else ['-o', str(output)]
    result = subprocess.run(command + [str(source)], capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise ValueError(f'{source.name} stream {index}: {result.stderr.strip() or result.stdout.strip()}')
    info = json.loads(result.stdout)
    if not isinstance(info, dict):
        raise ValueError(f'{source.name}: decoder returned invalid metadata')
    return info


def validate_wav(path: Path, info: dict) -> dict:
    try:
        wav = wave.open(str(path), 'rb')
    except (wave.Error, EOFError) as error:
        raise ValueError(f'{path.name}: invalid WAV output') from error
    with wav:
        channels, rate, frames = wav.getnchannels(), wav.getframerate(), wav.getnframes()
        if (wav.getsampwidth() != 2 or wav.getcomptype() != 'NONE'
                or channels not in (1, 2) or rate <= 0 or frames <= 0
                or channels != info['channels'] or rate != info['sampleRate']
                or frames != info['numberOfSamples']):
            raise ValueError(f'{path.name}: decoded PCM dimensions do not match the source')
        # Check the actual payload, not only a potentially truncated RIFF header.
        remaining = frames
        while remaining:
            count = min(remaining, 65536)
            if len(wav.readframes(count)) != count * channels * 2:
                raise ValueError(f'{path.name}: truncated PCM data')
            remaining -= count
    return {'channels': channels, 'sample_rate': rate, 'frames': frames,
            'seconds': frames / rate}


def audition_page(clips: list[dict]) -> str:
    rows = []
    for clip in clips:
        label = html.escape(f"{clip['bank']} / {clip['subsong']}")
        path = html.escape(clip['file'], quote=True)
        loop = 'looped source' if clip['loop'] else 'one-shot source'
        rows.append(f'<tr><td>{label}</td><td>{clip["seconds"]:.2f}s · {loop}</td>'
                    f'<td><audio controls preload="none" src="{path}"></audio></td></tr>')
    notes = ''.join(f'<li>{html.escape(note)}</li>' for note in LIMITATIONS)
    return ('<!doctype html><html lang="en"><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width,initial-scale=1">'
            '<title>Skate audio audition</title><style>'
            'body{font:16px system-ui;background:#141a21;color:#e8edf2;max-width:1100px;margin:40px auto;padding:16px}'
            'table{width:100%;border-collapse:collapse}td,th{text-align:left;padding:10px;border-bottom:1px solid #39424d}'
            'audio{width:280px}li{margin:8px 0}</style>'
            '<h1>Original skating sounds</h1><p>Local files from your own game. '
            'Each clip plays once. IDs identify source subsongs, not confirmed gameplay events.</p>'
            f'<ul>{notes}</ul><table><thead><tr><th>Bank / subsong</th><th>Duration</th><th>Listen</th></tr></thead><tbody>'
            + ''.join(rows) + '</tbody></table></html>')


def prepare(game: Path, output: Path, decoder: str, banks=BANKS) -> dict:
    game = source_directory(game, require_core=False)
    output = output.resolve()
    if output.exists():
        raise FileExistsError(f'Refusing to overwrite {output}; choose a new --output directory')
    archive = BigArchive(game / 'data/audio/audiofiles.big')
    by_name = {entry.path: entry for entry in archive.entries}
    selected = [by_name[f'data/audio/{bank}'] for bank in banks]
    output.parent.mkdir(parents=True, exist_ok=True)
    manifest = {'version': 1, 'clips': [], 'limitations': LIMITATIONS}
    # Decode completely before publishing. A failed bank leaves no partial library.
    with (tempfile.TemporaryDirectory(prefix='.skating-audio-', dir=output.parent) as temporary,
          ThreadPoolExecutor(os.cpu_count()) as pool):
        staging = Path(temporary) / 'library'
        staging.mkdir()
        for entry in selected:
            source = Path(temporary) / Path(entry.path).name
            data = archive.read(entry)
            source.write_bytes(data)
            digest = hashlib.sha256(data).hexdigest()
            embedded = splc_streams(data) if source.suffix == '.bnk' else None
            first = {} if embedded else decoder_info(decoder, source)
            count = len(embedded) if embedded else first.get('streamInfo', {}).get('total', 0) or 1
            if not isinstance(count, int) or not 1 <= count <= 4096:
                raise ValueError(f'{source.name}: unreasonable subsong count {count!r}')
            folder = staging / source.stem
            folder.mkdir()
            print(f'{source.name}: decoding {count} clips', flush=True)

            def clip(index):
                # One decoder process per clip. The bank's clips run concurrently
                # and are all collected, in order, before the next bank starts.
                target = folder / f'{index:04d}.wav'
                provenance = {}
                decode_source, decode_index = source, index
                if embedded:
                    stream = embedded[index - 1]
                    decode_source = Path(temporary) / f'{source.stem}-{index:04d}.snr'
                    decode_source.write_bytes(data[stream.offset:stream.offset + stream.size])
                    decode_index = 1
                    provenance = {'embedded_offset': stream.offset, 'embedded_size': stream.size,
                                  'embedded_hash': f'{stream.source_hash:08x}'}
                info = decoder_info(decoder, decode_source, decode_index, target)
                pcm = validate_wav(target, info)
                return {
                    'bank': source.name, 'archive': 'data/audio/audiofiles.big',
                    'entry': entry.path, 'source_sha256': digest, 'subsong': index,
                    'file': target.relative_to(staging).as_posix(),
                    'decoder_version': info['version'], 'encoding': info['encoding'],
                    'source_name': info.get('streamInfo', {}).get('name'),
                    'loop': info.get('loopingInfo'), **pcm, **provenance,
                }

            manifest['clips'].extend(pool.map(clip, range(1, count + 1)))
        (staging / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
        (staging / 'index.html').write_text(audition_page(manifest['clips']), encoding='utf-8')
        # Recheck to avoid replacing an output created during conversion.
        if output.exists():
            raise FileExistsError(f'Output appeared during conversion: {output}')
        staging.rename(output)
    return manifest


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('--output', type=Path, default=Path('.local/skating-audio'))
    parser.add_argument('--decoder', default='vgmstream-cli')
    args = parser.parse_args(argv)
    decoder = shutil.which(args.decoder)
    if decoder is None:
        parser.exit(1, 'vgmstream-cli not found. On macOS install it with: brew install vgmstream\n')
    try:
        manifest = prepare(args.game, args.output, decoder)
    except (OSError, RuntimeError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        parser.exit(1, f'Audio preparation failed: {error}\n')
    print(f"Decoded {len(manifest['clips'])} clips. Open {args.output.resolve() / 'index.html'}")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
