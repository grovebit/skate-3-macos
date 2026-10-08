"""Write the original collision-voice mix inputs into an impact library.

Writes material-impacts/mixmap/ (MixMapSK8.mxb, the volume/log/curve and pitch tables
from default.xex and the authored aud_general settings) without re-rendering
any clips. The runtime needs it beside collisions.json. Requires the xex2
package to decode default.xex.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import tempfile

from tools.asset_pipeline.setup_state import source_directory
from tools.owned_game import collision_mix
from tools.audio.prepare_impact_audio import owned_collections


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game', type=Path)
    parser.add_argument('--output', type=Path,
                        default=Path('.local/skating-audio/material-impacts/mixmap'))
    args = parser.parse_args()
    try:
        game = source_directory(args.game, require_core=False)
        with tempfile.TemporaryDirectory(prefix='.collision-mix-') as temp:
            collections, provenance = owned_collections(game, Path(temp))
            manifest = collision_mix.export(game, args.output.resolve(),
                                            collections['collections'], provenance)
    except (ValueError, OSError, RuntimeError) as error:
        parser.exit(1, f'{error}\n')
    print(f'Prepared the collision mix with settings {manifest["settings"]}')


if __name__ == '__main__':
    main()
