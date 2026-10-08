# Tools

Python tools that convert your copy of Skate 3 for the engine, plus audio
research and performance analysis. `play.sh` creates their environment in
`.local/venv-setup` (Python 3.13 with [`requirements-setup.txt`](requirements-setup.txt)).
Research tools and the Python tests also need
[`requirements-research.txt`](requirements-research.txt) (capstone).
It is a separate file because `requirements-setup.txt` is part of the
conversion fingerprints: changing it makes every player convert the game again.

```sh
uv pip install --python .local/venv-setup/bin/python -r tools/requirements-research.txt
```

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
  archives, RefPack, audio banks, MixMap programs, the verified
  `default.xex` image and more), shared by the
  pipeline and the audio tools. Its Python files are part of the conversion
  fingerprints too, so research-only code belongs elsewhere.
- [`native_replay/`](native_replay/): `ppc_interp.py` runs the PowerPC code
  of your `default.xex` (decoded by `owned_game/xex.py`) for the native
  verifiers.
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

Each `verify_*` tool replays original routines from your own `default.xex` in
`native_replay/ppc_interp.py` and compares the results with the production Rust
code. It checks the executable and its decoded image against the verified base
disc first; no game bytes are stored in the repository. Pass your game folder:

```sh
.local/venv-setup/bin/python -m tools.audio.verify_collision_pitch --game "/path/to/Skate 3"
```

## Performance

[`performance/`](performance/) analyzes frame-time reports and Chrome traces;
see [`docs/performance/`](../docs/performance/README.md).

## Tests

Run every Python test from the repository root, with the research requirements
installed:

```sh
.local/venv-setup/bin/python -m unittest $(git ls-files 'tools/test_*.py' \
    'tools/asset_pipeline/test_*.py' 'tools/audio/test_*.py' | sed 's/\.py$//; s#/#.#g')
(cd tools/mixamo_to_skate && ../../.local/venv-setup/bin/python -m unittest test_converter test_library)
```
