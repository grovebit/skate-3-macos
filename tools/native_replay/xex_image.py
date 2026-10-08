"""The memory-mapped image of your own base-disc default.xex.

Decoding needs the `xex2` package (https://github.com/landaire/acceleration,
MIT OR Apache-2.0), listed in tools/requirements-research.txt. Both the
executable and its decoded image are checked against the verified base disc,
so research addresses and replays always refer to the same build.
"""
from __future__ import annotations

import hashlib
from pathlib import Path

XEX_SHA256 = '1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f'
IMAGE_SHA256 = 'ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42'
# Virtual addresses equal IMAGE_BASE + offset in the mapped image.
IMAGE_BASE = 0x82000000


def extract(xex: bytes) -> bytes:
    """Decrypt and decompress default.xex into its memory-mapped image."""
    try:
        import xex2
    except ImportError as error:
        raise RuntimeError('Decoding default.xex needs the xex2 package '
                           '(pip install -r tools/requirements-research.txt)') from error
    return bytes(xex2.Xex2.parse(xex).extract_basefile())


def load(game: Path) -> bytes:
    """Return the verified mapped image of default.xex in the game folder."""
    path = Path(game) / 'default.xex'
    xex = path.read_bytes()
    if hashlib.sha256(xex).hexdigest() != XEX_SHA256:
        raise ValueError(f'{path} is not the verified base-disc executable')
    image = extract(xex)
    if hashlib.sha256(image).hexdigest() != IMAGE_SHA256:
        raise ValueError(f'{path} did not decode to the verified mapped image')
    return image
