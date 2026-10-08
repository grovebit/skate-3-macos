# Crates

| Crate | Role |
| --- | --- |
| [`skate-game`](skate-game/) | The game (`skate3rust`): Bevy app, rendering, HUD, character customiser, menus, audio, multiplayer and mod hosting. |
| [`skate-core`](skate-core/) | Recovered Skate 3 game logic with no rendering, I/O or ECS: physics, riding, air, player states, animation, camera, input, audio and scoring. |
| [`skate-data`](skate-data/) | Readers and validation for converted data: maps (`.skate`), collections, animation banks, state graphs, collision and scoring tables. |
| [`skate-dynamics`](skate-dynamics/) | A general rigid-body island on Rapier for mods; independent of the skateboard simulation. |
| [`skate-mods`](skate-mods/) | The Lua mod runtime (API 2) and `check_mod`, which validates a package. |
| [`skate-net`](skate-net/) | The transport-neutral multiplayer snapshot protocol. |
| [`skate-steam-relay`](skate-steam-relay/) | The optional Steam relay process; the game never links Steam itself. |

Dependencies point one way: `skate-game` uses every other crate except the
relay; `skate-data` and `skate-net` use `skate-core`; `skate-mods` uses
`skate-dynamics`; `skate-steam-relay` uses `skate-net`.

## skate-game

`src/main.rs` builds the app (`app.rs`) from the parsed configuration
(`config.rs`). `assets.rs` tracks loading of the converted game data, and
`difficulty.rs` selects the original physics-mode collection for the chosen
difficulty, with player-tuned values in `difficulty/custom.rs`. Everything else
is grouped by area:

| Folder | Area |
| --- | --- |
| `animation/` | The skater's stock graph, pose evaluation and graph host. |
| `camera/` | Gameplay and debug cameras. |
| `character/` | Character shading, the customiser and custom models. |
| `diagnostics/` | Crash reports, profiling, the `SKATE_PERF_REPORT` harness and startup verification. |
| `hud/` | The original APT HUD runtime, scoring HUD, session marker and FPS overlay. |
| `input/` | Controllers (Apple's GameController framework) and gesture mapping. |
| `menu/` | The settings and teleport menus. |
| `modding/` | The game side of the Lua mod API. |
| `multiplayer/` | Ten-player free skate over `skate-net`. |
| `physics/` | Scheduling the recovered simulation from `skate-core`. |
| `render/` | Retail world materials, sky, exposure, irradiance and presentation snapshots. |
| `replay/` | The rolling presentation replay. |
| `scoring_runtime/` | ScoreModule integration. |
| `skating_audio/` | Original skating sounds driven by simulation ticks. |
| `world/` | `.skate` maps, grind paths, map discovery, loading and transitions. |

Shaders (`.wgsl`) live beside the module that loads them, and `src/tests/`
holds tests and fixtures shared across modules.
