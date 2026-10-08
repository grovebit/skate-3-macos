# macOS (experimental)

Apple Silicon Macs run the game natively through Metal. This fork is
macOS-only: gameplay code is shared with upstream's Windows build, and the fork
differs in the renderer backend, controller input, window sizing and how game
data is prepared. It has no packaged release.

## Requirements

- An Apple Silicon Mac. `play.sh` asks to install the Xcode Command Line Tools
  and rustup, then fetches the rest at pinned versions, so every Mac builds and
  converts with the same tools: Rust from `rust-toolchain.toml`, and through
  `tools/pinned_tool.sh` uv, Python (`.python-version`), the hash-locked Python
  packages and vgmstream.
- Your own copy of Skate 3 for **Xbox 360**: its `.iso` disc image or an
  extracted game folder containing `default.xex` and `data/`. The PS3 version
  cannot be converted.
- A controller macOS supports: Xbox Wireless (Bluetooth), DualSense,
  DualShock 4 or Switch Pro. macOS does not support Xbox 360 controllers.

## Play

```sh
./play.sh                           # first run: choose the game in a Finder dialog
./play.sh "/path/to/Skate 3.iso"    # or name a game folder or .iso
./play.sh --map path/to/map.skate   # options are passed to the game
```

The first run converts the game into the gitignored `data/` folder with the
asset pipeline in `tools/asset_pipeline`. It takes about 2 minutes on an M3 Max
and uses about 3 GB. Intermediate files peak at about 5 GB. An `.iso` adds its size,
because it is extracted first (`tools/xiso.py`, no extra tools). Intermediate
files go to the system's temporary folder when the startup disk has room,
because an external data drive can be much slower to write; otherwise they go
beside `data/`. Setup validates every map by running the development build,
which `play.sh` builds first. `installation.json` records the
folder or `.iso` you chose; when a later version converts differently, the next
`./play.sh` refreshes only the affected asset groups from it and keeps your
settings. Naming a game always reruns that refresh.

Escape opens the settings menu and Option+Return toggles fullscreen. On laptop
keyboards the function keys (F6 replay, F9/F10 profiling) need Fn.

On Retina displays the default render scale is 50%, so the default 1280x800
window renders a 1280x800-pixel 3D image. Frames are presented with vsync (FIFO),
so the frame rate follows the display's refresh rate; set a lower limit in the
menu to save battery. Uncapped presentation is not offered: on a 120 Hz M3 Max
it stalled on drawable acquisition or skipped frames, for a median 78 FPS with
19 ms 99th-percentile frames and uneven on-screen pacing, against FIFO's steady
119.7 FPS and 9.2 ms.

## Check a controller

```sh
cargo run -p skate-game --bin skate3rust -- --pad-probe
```

This opens a window and prints each slot's buttons, triggers and sticks as they
change. It needs no game data.

## Original skating audio

Setup also decodes eight original sound banks into `.local/skating-audio`,
once, with [vgmstream](https://github.com/vgmstream/vgmstream) r2117 and a
static FFmpeg 9.0.2. `tools/pinned_tool.sh vgmstream` builds them from
checksummed sources with the Xcode Command Line Tools, in about 40 seconds on
an M3 Max, so every Mac decodes the same samples without Homebrew. If your
library is missing because an earlier setup skipped it, run
`./play.sh "/path/to/Skate 3"` again. To prepare or audition the library by hand:

```sh
tools/pinned_tool.sh vgmstream
.local/venv-setup/bin/python -m tools.audio.prepare_skating_audio "/path/to/Skate 3"
open .local/skating-audio/index.html
```

The library is a local WAV collection; the Xbox
360 copy tested here produces 1,761 clips: grinds, rolling rattles, flip tricks,
footsteps, wheel skids, foot drags, board scrapes and board collisions. The
collision bank's SPLC sample table is unpacked into embedded SNR streams before
decoding. The page lets you listen
to each bank/subsong. `manifest.json` records source hashes, decoder version,
sample dimensions and original loop points. WAVs play once, without added fades.

`play.sh` uses `.local/skating-audio` when present.
Restart the game after preparing the library. The initial mix uses original
rolling rattles and grind loops, plus airborne-trick, skid, footstep and impact
sounds. Fresh board and body solver contacts trigger impacts, including during
bails, with gain based on contact strength. Foot support has a higher trigger
threshold than board contacts. Hysteresis suppresses threshold chatter, while a
much harder follow-up collision can bypass the short debounce window. Quiet
support contacts do not hide eligible board impacts. Board and each body family
keep separate onset histories; at most two new families sound per simulation tick.
Thresholds and gain remain provisional host tuning.
With an impact library that includes `collisions.json` (below), head, torso,
arm and leg impacts instead come from the original body loop, with the
original speed graph on their strengths; see
[Runtime body-impact playback](../audio/body-impacts.md#runtime-body-impact-playback). The same file
also routes deck contacts through the original deck loop; trucks and feet keep
the provisional onset path.
Surface IDs and selected materials are logged as `SKATE_AUDIO_IMPACT`, and
original body records as `SKATE_AUDIO_BODY_RECORD`, and deck records as
`SKATE_AUDIO_DECK_RECORD`. Footsteps use short-attack subsongs
139–142 and horizontal travel rather than vertical body bobbing.
Rolling/grind loops fade in and out over 80 ms, with volume and pitch smoothed
as speed changes. Brief returns to a fading loop reuse it instead of restarting. Pause, replay, map loading,
teleports and mod-controlled vehicles stop the skating audio. Replay audio is
not reconstructed yet.

Set `SKATE3_AUDIO=/path/to/library` for another library. Set
`SKATE3_AUDIO_VOLUME=0` to mute, or a value between `0` and `1` to adjust the
mix (default `0.45`). These settings affect skating sounds, not mod audio.

For material-specific impacts, also run:

```sh
tools/pinned_tool.sh vgmstream
.local/venv-setup/bin/python -m tools.audio.prepare_impact_audio "/path/to/Skate 3"
```

This writes `material-impacts/` inside the local audio library. The tested disc
provides 89 surface routes and 305 impact variants mixed from 325 original clips
in `Skate_Collisions` and `Skate_Metal`. Board contacts select the contacted material; body contacts use
head, torso, arm, leg or foot samples according to the contacted physical part.
Older libraries without body-part routes retain their torso fallback.
`routes.json` records database and bank hashes,
material field/event IDs, decoded sample IDs, and each rendered variant’s source
layers, mix gain and playback trim. Playback trims reduce loud attacks using
the loudest 50 ms RMS window and peak level; they never boost quiet samples.
Impact logs include the selected source clip, trim and gameplay gain. Missing or invalid routes
fall back to the generic impact mix. Use `--output /path/to/library/material-impacts`
when preparing a different audio library.

The same export writes `collisions.json` for the original body loop. It holds
every audio material ID's bank, event matrix, base level, pitch and material
class (table `82FD1930`, `82484410`, `82484638`), the intensity and volume
profiles, the `aud_collisions/default` body and board cooldowns, bone/face
bands and body speed graph, and one rendered route for each referenced bank
event. The tested disc references 280
events: 163 in `Skate_Collisions`, 110 in `Skate_Metal` and 7 in `HOM_Set_1`.
Each manifest lists only its own clips, so `routes.json` stays within the
runtime's 512 KiB limit. Without `collisions.json`, body contacts use the
`routes.json` body-part samples as before.

The runtime reads only `collisions.json` version 2, which added the speed
graph. It treats an older file as absent and logs `invalid or outdated
collisions.json`. The exporter does not overwrite an existing folder, so
refresh a library by exporting beside it and swapping the folders:

```sh
.local/venv-setup/bin/python -m tools.audio.prepare_impact_audio "/path/to/Skate 3" \
    --output .local/skating-audio/material-impacts.new
mv .local/skating-audio/material-impacts .local/skating-audio/material-impacts.old
mv .local/skating-audio/material-impacts.new .local/skating-audio/material-impacts
```

The export also writes `mixmap/`: the owned `data/audio/MixMapSK8.mxb`, the
volume, log and curve tables from `default.xex`, and three `aud_general`
floats. Every file is checked against the verified base disc's SHA-256 before
any clip is rendered. Decoding `default.xex` needs the
[`xex2`](https://github.com/landaire/acceleration) package. The runtime
computes collision voice volume from these files as the original does; see
[Live collision volume controls](../audio/collision-volume.md#live-collision-volume-controls). A library
prepared before `mixmap/` existed can gain it without re-rendering clips:

```sh
python3 -m tools.audio.prepare_collision_mix "/path/to/Skate 3"
```

The runtime ignores `collisions.json` without a valid `mixmap/` and logs a
warning; body contacts then use the `routes.json` body-part samples. There is
no full-scale volume fallback.

Material events are linked through the original SPLC leaf/group/layer tables to
embedded samples; event IDs are not treated as WAV indices. Playback chooses
up to four variants from one representative event per material. The exporter
preserves each variant’s layers, selects one sample per layer, and mixes them
into one WAV. Layers start together, with equal-power attenuation and further
peak attenuation when needed. The longest layer’s tail is preserved; a single
layer retains its original PCM. Gameplay still uses one voice per impact.
Original layer timing, envelopes, pitch, randomizers and strength-dependent
event selection remain unfinished. Body-part sounds currently share the same
sample family across contacted surfaces.
Footstep timing is provisional; remaining foley banks and granular
wheel rolling still need work, so full wheel noise is absent.
The output stays in gitignored `.local/` and does not change the prepared
installation. Existing output is never overwritten; use `--output DIRECTORY`
for a new export. Without the library, the game runs without skating audio.

## Test local multiplayer

```sh
python3 scripts/multiplayer.py --two-controllers
```

This builds the game and starts two windows on University,
with player A hosting a loopback lobby. Steam is not required. The first two
controller slots control players A and B when `--two-controllers` is present;
omit it to use the game's default controller selection. Use `--players 2` through
`--players 10` to change the lobby size, `--map` for another converted map, or
`--assets` (also `SKATE3_ASSETS`) for a different installation. More windows need
more memory and GPU time.

Logs are written per player under `logs/multiplayer/session-*`. Keep the launcher
running; Ctrl+C stops all players. `--dry-run` validates the asset and map paths
and prints the launch commands without building or opening windows.

## Platform notes

- Rendering uses Metal. naga writes every gradient sample as `gradient2d`,
  which Metal rejects for cube arrays, so environment reflections sample an
  explicit mip level instead (`sample_environment` in
  `retail_material_bindings.wgsl`). Return to `textureSampleGrad` once Bevy
  ships a naga that emits `gradientcube`.
- Metal compiles each shader the first time it is used, so expect brief
  stutter when something first appears.
- If the game crashes, a native dialog offers to open its diagnostic report in
  your default text editor. Reports are saved in
  `~/Library/Logs/Skate3RustEngine/CrashReports` (or the temporary directory if
  that location is unavailable). Nothing is uploaded automatically. Run
  `cargo run -p skate-game --bin skate3rust -- --crash-report-preview` to try the
  dialog without game data.
- Upstream's Windows setup and update helpers, release publishing and custom
  character importer are not part of this fork. Pros, specials and models
  already in `~/Library/Application Support/Skate3RustEngine/custom-characters`
  can still be chosen under Custom models.
- The game looks for the Steam relay as `skate-steam-relay` and
  `libsteam_api.dylib` in a `steam-relay` folder beside its binary. Nothing
  stages them yet, so Steam multiplayer is unavailable; direct and local
  sessions don't need it.
- Development builds link Bevy dynamically and record where its libraries live,
  so `target/debug/skate3rust` runs directly as well as through `cargo run`.

## Research notes

- [Audio research](../audio/README.md): how the original game's audio works and what
  the port reproduces.
- [Performance records](../performance/README.md): frame-time, GPU and CPU
  measurements.
