"""Original collision-voice mix inputs: MixMap program, tables and settings.

The runtime evaluates the MixMap dependency cone of the collision volume and pitch
outputs (base-disc evaluator 82927E90). It needs the owned program
data/audio/MixMapSK8.mxb, five lookup tables from the mapped default.xex
image (volume 82FBB430, log 82FBBD98, curve 82FBC598) and three authored
aud_general floats. Every input is checked against the hash of the
verified base disc; nothing is generated or approximated.

Decoding default.xex needs the `xex2` package
(https://github.com/landaire/acceleration, MIT OR Apache-2.0).
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import struct

XEX_SHA256 = '1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f'
PROGRAM = ('MixMapSK8.mxb', '105f46bbc4ae25cf51bafc8524e00b0aef608a3aa91424e3d39bbc57fff305c2')
IMAGE_BASE = 0x82000000
# name: (mapped address, size, SHA-256)
TABLES = {
    'pitch_semitones': (0x82FBCD9C, 48,
        'c13e2d9beaea71c36f537d765a5acf32e9ae677115e623c70bb1d0aa6c15401b'),
    'pitch_cents': (0x82FBCDD0, 400,
        'a99305d3b37ac173558b6dcef85a3660093fa59f2e41608d6ca6c5871fea335d'),
    'volume': (0x82FBB430, 2408,
               '30c4769614bc8837f9016bc6698826a1c347f4f124b0db1e4cef314748609e65'),
    'log': (0x82FBBD98, 2048,
            '86bdf6292119e406a7f06df1932cbfc69d2321c0cf4340cb51bf1ba80c18934f'),
    'curve': (0x82FBC598, 2052,
              'be693b485c8b528a278c1105000863624b21ffca877cfb2bc52c62b8684e905b'),
}
# (row, field): 3DColPos controller +70 (8249BEF8), Master treatment scale
# (824C2E94..824C2EE4) and the HOM activation threshold (82476A0C..82476A64).
SETTINGS = {
    'listener_offset': ('camera', 'Hash_A6F853B935E46E5F'),
    'treatment_scale': ('treatments', 'Hash_A12258EB71B6937A'),
    'hom_threshold': ('hom', 'Hash_4BD3E0CCEA90BE26'),
}


def _sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def executable_tables(xex: bytes, extract=None) -> dict[str, bytes]:
    """Slice the five tables from the mapped base-disc image.

    `extract` maps raw XEX bytes to the mapped image; by default the
    xex2 package performs the decryption and decompression.
    """
    if _sha(xex) != XEX_SHA256:
        raise ValueError('default.xex is not the verified base-disc executable')
    if extract is None:
        try:
            import xex2
        except ImportError as error:
            raise RuntimeError('Decoding default.xex needs the xex2 package '
                               '(pip install xex2)') from error
        extract = lambda raw: bytes(xex2.Xex2.parse(raw).extract_basefile())
    image = extract(xex)
    tables = {}
    for name, (address, size, digest) in TABLES.items():
        offset = address - IMAGE_BASE
        blob = image[offset:offset + size]
        if len(blob) != size or _sha(blob) != digest:
            raise ValueError(f'{name} table does not match the verified executable')
        tables[name] = blob
    return tables


def settings(rows) -> dict[str, float]:
    result = {}
    for name, (key, field) in SETTINGS.items():
        matches = [r for r in rows if r['class'] == 'aud_general' and r['key'] == key]
        if len(matches) != 1:
            raise ValueError(f'Expected one aud_general/{key} row')
        value = matches[0]['fields'].get(field)
        if value is None or value['type'] != 'EA::Reflection::Float':
            raise ValueError(f'aud_general/{key}: missing or mistyped {field}')
        raw = bytes.fromhex(value['data'])
        if len(raw) != 4:
            raise ValueError(f'aud_general/{key}: invalid {field}')
        number = struct.unpack('>f', raw)[0]
        if number != number or number in (float('inf'), float('-inf')):
            raise ValueError(f'aud_general/{key}: nonfinite {field}')
        result[name] = number
    return result


def inputs(game: Path, rows, extract=None) -> tuple[bytes, dict[str, bytes], dict[str, float]]:
    """Read and verify the owned program, executable tables and settings."""
    program = (game / 'data/audio' / PROGRAM[0]).read_bytes()
    if _sha(program) != PROGRAM[1]:
        raise ValueError(f'{PROGRAM[0]} is not the verified base-disc program')
    tables = executable_tables((game / 'default.xex').read_bytes(), extract)
    return program, tables, settings(rows)


def write(output: Path, program: bytes, tables: dict[str, bytes], values: dict[str, float],
          provenance: list[dict]) -> dict:
    """Write mixmap.json and its verified inputs into a new directory."""
    if output.exists():
        raise FileExistsError(f'Refusing to overwrite {output}')
    output.mkdir(parents=True)
    (output / PROGRAM[0]).write_bytes(program)
    files = {'program': PROGRAM[0]}
    hashes = {PROGRAM[0]: _sha(program)}
    for name, blob in tables.items():
        file = f'{name}.bin'
        (output / file).write_bytes(blob)
        files[name] = file
        hashes[file] = _sha(blob)
    manifest = {
        'version': 1,
        'files': files,
        'settings': values,
        'source': {'default.xex': XEX_SHA256, 'files': hashes, 'database': provenance},
        'limitations': ['Owned program and executable tables for the collision volume and pitch '
                        'MixMap cone; producer inputs are supplied by the runtime.'],
    }
    (output / 'mixmap.json').write_text(json.dumps(manifest, indent=1) + '\n')
    return manifest


def export(game: Path, output: Path, rows, provenance: list[dict], extract=None) -> dict:
    """Read the owned program and default.xex beside it, then write `output`."""
    return write(output, *inputs(game, rows, extract), provenance)
