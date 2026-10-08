"""The original PCA ocean/water animation table (assets/private/ocean-pca.json).

cPCAWaterAnimationData::Init (base disc 8276D120) points the animation at 30
frames of means (82FC3978, 3 floats per frame) and the weights that follow
them (82FC3AE0, 24 floats per frame). Its update (8276D3C8) scales the
selected frame by 1/255 (8213F07C) into seven shader constants, in this
order: g_fPcaMean (fourth component 1.0, 82314D90), g_fPcaWeightsR_0/_1,
G_0/_1 and B_0/_1. Each frame is written as those seven rows, rounded to
single precision like the original. Without this file the ocean and
still-water families render as family 1. The executable and the table are
checked against the verified base disc; no game content is embedded here.
"""
import json
import struct
from pathlib import Path

from tools.owned_game import xex

FRAMES = 30
# Means then weights, 27 floats per frame: one contiguous verified table.
TABLE = (0x82FC3978, FRAMES * 27 * 4, '6febf907c06ae35dc1d7d6b366bd0fab56783cb52bcdc8bc8dd67163136a162a')
SCALE = struct.unpack('>f', bytes.fromhex('3B808081'))[0]  # 1/255 at 8213F07C


def single(value):
    """Round to binary32. A product of two binary32 values is exact in a
    double, so this reproduces the original `fmuls`."""
    return struct.unpack('>f', struct.pack('>f', value))[0]


def frames(image):
    values = struct.unpack(f'>{FRAMES * 27}f', xex.table(image, 'Ocean PCA', *TABLE))
    means, weights = values[:FRAMES * 3], values[FRAMES * 3:]
    result = []
    for frame in range(FRAMES):
        mean = [single(v * SCALE) for v in means[frame * 3:frame * 3 + 3]] + [1.0]
        row = weights[frame * 24:frame * 24 + 24]
        result.append([mean] + [[single(v * SCALE) for v in row[i:i + 4]] for i in range(0, 24, 4)])
    return result


def convert(game, assets):
    """Write assets/private/ocean-pca.json from default.xex in the game folder."""
    rows = frames(xex.load(game))
    output = Path(assets) / 'private/ocean-pca.json'
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps({'source_sha256': xex.XEX_SHA256, 'frames': rows}), encoding='utf-8')
    return len(rows)


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path, help='your Skate 3 folder, with default.xex')
    parser.add_argument('assets', type=Path, help='the installation assets directory')
    args = parser.parse_args()
    print('Extracted', convert(args.game, args.assets), 'ocean PCA frames')
