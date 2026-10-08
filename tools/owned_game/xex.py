"""The memory-mapped image of your own base-disc default.xex and its tables.

Setup reads executable tables from it and the native verifiers replay its
code. Both the executable and its decoded image are checked against the
verified base disc, so every table and research address refers to the same
build; addresses are mapped-image addresses (base 82000000). Decoding needs
the `xex2` package (https://github.com/landaire/acceleration, MIT OR
Apache-2.0), listed in tools/requirements-setup.txt.
"""
from __future__ import annotations

import hashlib
from pathlib import Path

XEX_SHA256 = '1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f'
IMAGE_SHA256 = 'ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42'
# Virtual addresses equal IMAGE_BASE + offset in the mapped image.
IMAGE_BASE = 0x82000000


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def extract(xex: bytes) -> bytes:
    """Decrypt and decompress default.xex into its memory-mapped image."""
    try:
        import xex2
    except ImportError as error:
        raise RuntimeError('Decoding default.xex needs the xex2 package '
                           '(pip install -r tools/requirements-setup.txt)') from error
    return bytes(xex2.Xex2.parse(xex).extract_basefile())


def load(game: Path) -> bytes:
    """Return the verified mapped image of default.xex in the game folder."""
    path = Path(game) / 'default.xex'
    xex = path.read_bytes()
    if sha256(xex) != XEX_SHA256:
        raise ValueError(f'{path} is not the verified base-disc executable')
    image = extract(xex)
    if sha256(image) != IMAGE_SHA256:
        raise ValueError(f'{path} did not decode to the verified mapped image')
    return image


def table(image: bytes, name: str, address: int, size: int, digest: str) -> bytes:
    """Slice one table from the mapped image and check its verified SHA-256."""
    offset = address - IMAGE_BASE
    blob = image[offset:offset + size] if offset >= 0 else b''
    if len(blob) != size or sha256(blob) != digest:
        raise ValueError(f'{name} table does not match the verified executable')
    return blob
