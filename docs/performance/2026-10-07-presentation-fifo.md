# Presentation pacing and FIFO vsync, 2026-10-07

Integration branch `main` at `76d8955`; the fix merged as `aa865cf`
(`3adc278` Present with vsync instead of uncapped AutoNoVsync). Apple M3 Max,
120 Hz built-in display, University, saved settings (1920x1080 points, 100%
render scale, no FPS limit; about 3840x2102 rendered), repository mods and
original skating audio.

Release builds, SHA-256:
`181046a7e410f2ef263b1ba75d0ea9c2c1e2ec980fc5d7d79324db48a7908a28` (`main`,
AutoNoVsync) and `49f542d6eda964d3efc6712b1ef3a423e4dfca24b2637f7c1c4d7bc2393ea6bc`
(FIFO plus the frame-state buffer fix).

## Method

`SKATE_PERF_REPORT` (10 s warmup, 15 s sample) in alternating rounds, plus the
system's always-on Metal statistics retrieved afterwards for each process:

```sh
metalperftrace collect --start <UTC> --json <dir>
metalperftrace overview --json <dir>/<trace>.atrc
```

The overview's per-layer "Presented Frame Stats" give GPU time, drawable wait
and CPU latency; "Frame-On-Glass Interval Stats" give on-screen pacing. Check
`pmset -g batt` first: battery power lowers GPU clocks and changes the uncapped
results.

## Findings

- Frames were bound by drawable acquisition, not CPU or GPU work. With
  `SKATE_PERF_RENDER=1`, ManageViews (`prepare_windows` ->
  `CAMetalLayer nextDrawable`) took 9.8 ms of a 12.9 ms render frame; extract,
  asset preparation, queueing, preparation and graph encoding took 0.1, 0.3,
  0.05, 0.9 and 1.4 ms. The main schedule took about 2.7 ms.
- Uncapped `AutoNoVsync` alternated between two regimes: about 77 FPS with
  ~9 ms drawable waits and on-glass intervals quantized to 8.33/16.67 ms, or
  about 125–140 FPS presented with 47–90 skipped frames and on-glass intervals
  from 0.2 ms upward.
- The existing sleep-based 120 FPS limiter did not help (84 FPS median).
- Release and dev builds (`opt-level=3`, dynamic Bevy) gave the same frame rate;
  the release main schedule was about 10% cheaper. `-C target-cpu=native` adds
  only `bf16`, `bti` and `i8mm` over the default Apple target.
- The frame-state buffer fix (one GPU buffer written in place instead of a new
  buffer every frame) had no measurable frame-time effect.
- About 10% of harness exits hung in Bevy 0.18's shutdown: winit's `exiting`
  drops the windows, `RenderAppChannels::drop` blocks the main thread, and the
  render thread waits for `create_surfaces`, the only main-thread render system
  on macOS. This happens on `main` too and is not fixed.

## Results

Eight alternating rounds on AC power, medians (`main` -> FIFO):

| Metric | AutoNoVsync | FIFO |
| --- | ---: | ---: |
| FPS | 78.2 (71–125) | 119.7 (119.4–119.8) |
| Frame-time SD | 3.46 ms | 0.65 ms |
| 99th-percentile frame | 19.2 ms | 9.2 ms |
| On-glass interval SD | 4.51 ms | 1.28 ms |
| Skipped frames per run | 6.5 | 2 |
| GPU time per frame | 6.3 ms | 4.9 ms |
| Drawable wait | 9.2 ms | 4.9 ms |

Three rounds on battery: 76.2 -> 118.9 FPS, 99th percentile 19.0 -> 13.2 ms.
