# University gameplay CPU baseline, 2026-10-05

Integration branch: `main`, starting commit `dc171c33d5d95c1f34ce8fb423f07fc3ed402bf4`.
Task branch: `task/performance-profile`.
Executable SHA-256: `ca57a0c6199706fae1396d453072425f1e21dae625ef24a9b0c706d50fe55e01`.

## Capture

Apple M3 Max, Metal, optimized development build (`opt-level=3`, debug symbols,
dynamic Bevy). Instruments 27 Time Profiler attached to gameplay child PID
61562. University map, easy difficulty, original audio and repository mods.
The window was already open before attachment. Recording ran from
17:24:05.667 UTC to 17:25:17.421 UTC (71.753 seconds), ending when the app exited,
before the requested five-minute limit. Final logs show the game paused.
This is a mixed interactive baseline, not a repeatable benchmark.

Raw capture and derived exports remain local in
`/private/tmp/skate-3-performance-profile/`:
`gameplay-cpu.trace`, `game-session.log`, `cpu-samples.xml`, `cpu-summary.json`,
`running-summary.json`, and `source-summary.json`. They are not committed.
Instruments also records process environment metadata; review before sharing.

## Results

166,196 samples had nonempty stacks, weighted at 1 ms each (166.196 seconds
across all threads). Percentages below count samples containing a function
or subsystem anywhere in the stack; callers overlap, so rows do not sum to
100%. These are sampled CPU stacks, not frame-time shares or GPU timings.
Synchronization and system-call frames are present even though the export
labels the samples Running; do not equate every weighted sample with useful
application computation.

| Stack contains | Share of nonempty weighted samples |
| --- | ---: |
| Bevy rendering | 45.70% |
| Bevy PBR | 22.59% |
| Metal functions | 20.02% |
| Apple AGX driver functions | 15.02% |
| `render_system` | 10.05% |
| `RenderDevice::create_buffer_with_data` | 6.18% |
| HUD material `prepare_assets` | 3.08% |
| Modding `update` | 2.42% |
| Modding `snapshot_ro` | 2.20% |
| SkinStamp material `prepare_erased_assets` | 2.06% |
| Mesh allocator `allocate_and_free_meshes` | 1.93% |
| CharacterMaterial `prepare_erased_assets` | 1.82% |
| Modding player-physics `snapshot` | 1.68% |
| Gameplay physics `advance` | 1.55% |
| Tracing subscriber | 2.12% |

## First optimization candidates

1. Avoid redundant HUD mesh/material writes. `scoring_hud.rs` replaces both
   assets for every existing draw slot each frame. `Assets::get_mut` emits a
   modification, causing renderer preparation even when the new output is
   identical. Compare exact geometry, color transform, and atlas data before
   mutating assets. Keep the original APT runtime, draw order, visibility,
   frame advancement, and output unchanged. Do not skip authored animation.
2. Inspect mod snapshots for allocations or work that no active consumer
   needs. `snapshot_ro` and player-physics snapshot creation are visible
   hotspots; this capture does not establish that their contents are unused.
3. Profile character-material uploads before changing them. Lighting currently
   smooths SH values as the player moves; many updates are real. Existing
   unchanged-value checks already exist. Do not add approximate thresholds
   or reduce update frequency without checking observable lighting behavior.

No runtime optimization has been applied and no FPS improvement is claimed.
A CPU-only capture cannot determine whether the frame rate is GPU-limited.
Validate any change with a comparable route, frame-time statistics, and a
Metal capture when needed. A before/after test should use the same build mode,
resolution, settings, map, audio, mods, and thermal conditions. Preserve exact
original gameplay output; treat cache invalidation as host integration work.
