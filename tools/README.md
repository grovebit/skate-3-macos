# Tools

Python tools that convert your copy of Skate 3 for the engine, plus audio
research and performance analysis. Their environment is `.local/venv-setup`,
which `play.sh` creates with [`pinned_tool.sh`](pinned_tool.sh): the Python
version in [`.python-version`](../.python-version) and the hash-locked
[`requirements-setup.txt`](requirements-setup.txt). To change a package, edit
its version there and rerun the command in the file's header.
Research tools and the Python tests also need
[`requirements-research.txt`](requirements-research.txt) (capstone).
It is a separate file because `requirements-setup.txt` is part of the
conversion fingerprints: changing it makes every player convert the game again,
unless `asset_pipeline/pipeline-equivalence.json` records the change as equivalent.

```sh
"$(tools/pinned_tool.sh uv)" pip install --python .local/venv-setup/bin/python -r tools/requirements-research.txt
```

`pinned_tool.sh` also provides the pinned uv and vgmstream; it prints the
tool's path after downloading or building it once. Downloads must match their
pinned SHA-256.

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
is in [`docs/audio/`](../docs/audio/README.md). The library builders decode with
the vgmstream that `tools/pinned_tool.sh vgmstream` builds: vgmstream r2117 with
a static FFmpeg 9.0.2, from checksummed sources.

Each `verify_*` tool replays original routines from your own `default.xex` in
`native_replay/ppc_interp.py` and compares the results with the production Rust
code. It checks the executable and its decoded image against the verified base
disc first; no game bytes are stored in the repository. Pass your game folder:

```sh
.local/venv-setup/bin/python -m tools.audio.verify_collision_pitch --game "/path/to/Skate 3"
```

## Performance

[`performance/`](performance/) measures the game and analyzes the results; see
[`docs/performance/`](../docs/performance/README.md).

- `bench.sh` runs builds in alternating rounds through the `SKATE_PERF_REPORT`
  harness, optionally under the Time Profiler or a Metal System Trace.
- `metal_stats.py` reads macOS's always-on Metal statistics for those runs:
  process instructions, missed vblanks, GPU time and drawable waits.
- `time_profile.py` summarizes a Time Profiler capture per thread, with Rust
  symbols demangled; `metal_trace.py` gives GPU time per render pass.
- `analyse_performance_trace.py` and `parse_trace.py` read the engine's own
  Chrome traces (`--trace`).

## Tests

Run every Python test from the repository root, with the research requirements
installed:

```sh
.local/venv-setup/bin/python -m unittest $(git ls-files 'tools/test_*.py' \
    'tools/asset_pipeline/test_*.py' 'tools/audio/test_*.py' | sed 's/\.py$//; s#/#.#g')
(cd tools/mixamo_to_skate && ../../.local/venv-setup/bin/python -m unittest test_converter test_library)
```
