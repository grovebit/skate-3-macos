# Runtime profile: fullscreen flythrough, 2026-10-08

Integration branch `main` at `6c559ee`; task branch `task/perf-profile`. Apple
M3 Max, 120 Hz built-in display, on battery (50–51%). University, saved
settings at 100% render scale, borderless fullscreen (3456x2168 drawable),
repository mods (Skyline Drive) and skating audio. Development builds, as
`./play.sh` runs them. Raw reports, logs, traces and the compared binaries are
in `.local/performance-raw/2026-10-08-runtime-profile/`.

## Method

The `SKATE_PERF_REPORT` harness now measures the game fullscreen and moving:
after a 10 s warmup, the camera flies once around a 4,890 m loop through
University's ten travel destinations in 60 s, 3 m above the topmost collision
surface (`diagnostics/flythrough.rs`). The skater stays at the spawn. An earlier
version rode the skater straight ahead instead; it bailed on the same ledge
every 10 s and covered 90 m.

Builds ran in interleaved rounds with `tools/performance/bench.sh`. Pacing,
GPU time, latency and whole-process instructions come from macOS's always-on
Metal statistics (`metal_stats.py`, no profiler attached). Hotspots come from
one 55 s Time Profiler capture (`time_profile.py`) and one 5 s Metal System
Trace (`metal_trace.py`); both slow the game (106 and 114 FPS) and attribute
work only.

## Baseline

Frames held 119.4–119.8 FPS with 3–20 hitches per minute. The hitches recur at
the same points of the route with normal main-schedule times; per-second GPU
maxima were mostly 6.3–8.1 ms, against the 8.33 ms vblank.

- **GPU, about 5 ms per frame.** In one traced frame (4.87 ms): the main opaque
  pass took 3.1 ms (0.91 vertex, 2.20 fragment), the five shadow cascades
  0.65 ms, and presentation about 0.9 ms: tone mapping 0.21, the scene camera's
  copy into its image 0.18, the UI pass drawing that image and the HUD 0.28,
  the window copy 0.07, and the session-marker camera's copy 0.11 ms though it
  drew nothing. Vertex intervals that overlap another pass's fragment work
  (the 2D passes, the transparent pass) are waits, not work.
- **78 Metal command buffers per frame.** Submitting them took 7.5 s of the
  render thread and Metal's queue threads in the 55 s capture, and finishing
  encoders 4.8 s. The three 2D cameras' opaque passes, which draw almost
  nothing, cost 1.86 s of encoding against 0.40 s for the world's opaque pass.
- **Main thread, 2.6 ms per frame.** The mod runtime took 37% of it. Each frame
  with a physics tick built the native mod snapshot twice, once for
  `on_fixed_update` and again after the mods' commands for `on_update`; 42% of
  each build was `serde_json`'s `json!` serializing already-built arrays into
  deep copies. Render-world extraction took 16%.
- **Compute task pool.** Eight threads used 73.6 s of CPU in 55 s; 22.5 s of it
  inside mutex and condition-variable calls of Bevy's executor. The customiser
  re-resolved the applied outfit every frame (1.09 s, 89% of it in
  `Parts::resolve`) only to find nothing to do.
- **Tracing.** Bevy's per-system spans cost 2.0–2.6% of each thread's CPU
  outside a `--trace` capture.

## Changes

Merged; behaviour and output are unchanged by construction:

1. Mod snapshots move built values into the result instead of serializing
   copies (`modding/player_physics.rs`, `modding/mod.rs`). serde_json is built
   without `preserve_order`, so key order is sorted either way.
2. The customiser returns before resolving when the closed customiser's preview
   equals the applied outfit. The library is fixed after startup and an outfit
   is applied only after it resolved, so the skipped resolve could not fail.
3. Span callsites are disabled unless `--trace` records them; logs lose their
   `system{name=…}` prefixes.

Measured but not merged (`task/overlay-camera-rest`, commit `51094df`): the
session-marker camera running only while its HUD or screen effect shows, and
the scoring HUD camera and composite resting while the movie draws nothing.
Neither changes the image: the marker camera only re-copied the presented
frame, and the HUD target would be cleared to transparent black, which the
premultiplied composite leaves unchanged.

## Results

Four interleaved rounds of each build in one session: the baseline, the merged
changes, and those plus the camera rests. Ranges are per run.

| Metric | Baseline | Merged | Plus camera rests |
| --- | ---: | ---: | ---: |
| Process instructions (G) | 529–554 (mean 544) | 483–519 (498) | 459–474 (465) |
| Process CPU (s) | 107–113 (110) | 98–109 (103) | 90–95 (93) |
| Main schedule median (ms) | 2.70–2.75 | 2.43–2.53 | 2.28–2.32 |
| GPU per frame (ms) | 5.09–5.15 | 4.92–5.10 | 5.00–5.09 |
| Command buffers per frame | 78 | 78 | 57 |
| Missed vblanks in 4 min | 105 | 153 | 87 |
| Harness hitches in 4 min | 42 | 60 | 18 |
| End-to-end latency (ms) | 22.0 | 22.4 | 25.7 |

The merged changes cut instructions by 8%, process CPU time by 6% and the main
schedule median by 0.25 ms. GPU time and pacing are unchanged within the noise:
missed vblanks and hitches vary several-fold between runs of one build.

The camera rests would cut instructions by a further 7%, but end-to-end
latency (a frame's first command buffer to its presentation) rose about 3 ms:
frames finish sooner but wait longer before display, and two to three times as
many presents reach the glass late. Three rounds isolating each rest against
the merged build put the rise on the session-marker camera (+2.1, +3.4 and
+5.3 ms; the HUD rest gave +3.2, +0.1 and −5.6 ms). The merged changes add
0.1–1.0 ms against the baseline in each round. Under FIFO without frame pacing,
a render thread with less to do runs further ahead of the display and its
frames queue longer. This measure starts at the first command buffer, not at
input sampling, so the effect on input latency is inferred.

## Not changed

- The main opaque pass is the retail world shader; its cost is the port's.
- The scene sun renders four cascades out to 100 m with every world mesh as a
  caster, for character and PBR receivers.
- Both mod snapshot builds per tick stay: mods' commands between them can
  change what the second one reports.
- Bevy's executor overhead and per-pass command buffers.
- Frame pacing, which the camera rests would need first.
- 8 of 44 fullscreen harness runs hit the known Bevy 0.18 shutdown deadlock
  after writing their report (`bench.sh` stops them).
