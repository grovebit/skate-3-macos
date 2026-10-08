# Tools

Python tools that convert your copy of Skate 3 for the engine, plus audio
research and performance analysis. `play.sh` creates their environment in
`.local/venv-setup` (Python 3.13 with [`requirements-setup.txt`](requirements-setup.txt)).

## Game data conversion

[`prepare_assets.py`](prepare_assets.py) is the entry point `play.sh` runs. It
accepts a game folder or an `.iso` ([`xiso.py`](xiso.py) extracts disc images)
and drives [`asset_pipeline/`](asset_pipeline/), which converts maps,
characters, the HUD and environment data into `data/`. Its fingerprints
(`asset_pipeline/versions.py`) decide what a later run must rebuild.

The exporters the pipeline runs by path stay at this level, because their paths
are part of those fingerprints:

- HUD: `prepare_hud.py`, `prepare_runtime_huds.py`, `install_prepared_hud.py`,
  `extract_session_marker.py`.
- Default skater: `extract_default_skater.py`, `default_skater_retail_manifest.json`,
  `apply_default_skater_materials.py`, `add_onboard_ik_targets.py`,
  `export_bevy_glb.py`.

Other folders:

- [`owned_game/`](owned_game/): readers for the game's own formats (BIG
  archives, RefPack, audio banks, MixMap programs and more), shared by the
  pipeline and the audio tools.
- [`mixamo_to_skate/`](mixamo_to_skate/): converts Mixamo characters for the
  in-game Custom models menu. `preview_mixamo_animation.py` reviews a raw
  Mixamo animation in Blender.
- [`vendor/`](vendor/): third-party extraction code, under its own licenses.
- `make_skate_demo.py` regenerates `maps/format-demo.skate`.

## Audio

[`audio/`](audio/) holds the original-audio library builders
(`prepare_skating_audio.py`, `prepare_impact_audio.py`,
`prepare_collision_mix.py`), the research checks and native-code verifiers
(`check_*`, `inspect_*`, `verify_*`) and their tests. The research they support
is in [`docs/audio/`](../docs/audio/README.md).

## Performance

[`performance/`](performance/) analyzes frame-time reports and Chrome traces;
see [`docs/performance/`](../docs/performance/README.md).

## Tests

Run every Python test from the repository root:

```sh
.local/venv-setup/bin/python -m unittest $(git ls-files 'tools/test_*.py' \
    'tools/asset_pipeline/test_*.py' 'tools/audio/test_*.py' | sed 's/\.py$//; s#/#.#g')
(cd tools/mixamo_to_skate && ../../.local/venv-setup/bin/python -m unittest test_converter test_library)
```
