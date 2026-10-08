# Performance records

Measurements on an Apple M3 Max, newest first. Each record names its build,
method and limits; JSON files hold the compact data behind a record. Commit
hashes in records dated before 2026-10-07 refer to the history before the fork
was squashed, kept locally on the `backup/pre-squash-6a9c7db` branch.

| Record | Summary |
| --- | --- |
| [2026-10-08-setup-conversion](2026-10-08-setup-conversion.md) | Game conversion was mostly single-threaded or waiting on a slow data drive; parallel stages and a temporary folder on the startup disk cut it from 340 s to 120 s. |
| [2026-10-08-hidden-board-world-query](2026-10-08-hidden-board-world-query.md) | A hidden or returning board no longer walks every map triangle each tick: about 1 ms per query on University before, at most 20 ns after. |
| [2026-10-07-presentation-fifo](2026-10-07-presentation-fifo.md) | Frames waited on drawable acquisition; FIFO vsync gives a steady 119.7 FPS instead of 78 FPS with uneven pacing. |
| [2026-10-05-native-only-timing](2026-10-05-native-only-timing.md) | Four minutes of native Metal timing without Instruments; slow intervals persist without a profiler attached. |
| [2026-10-05-drawable-acquisition-stall](2026-10-05-drawable-acquisition-stall.md) | A focused slowdown traced to `nextDrawable` waits rather than CPU work. |
| [2026-10-05-slowdown-native-metal](2026-10-05-slowdown-native-metal.md) | A late slowdown recovered from always-on Metal statistics. |
| [2026-10-05-hud-upload-optimization](2026-10-05-hud-upload-optimization.md) | Unchanged scoring HUD meshes and materials are no longer re-uploaded. |
| [2026-10-05-gameplay-gpu](2026-10-05-gameplay-gpu.md) | Short Metal System Trace baseline: GPU active about 3.9 ms per frame. |
| [2026-10-05-gameplay-cpu](2026-10-05-gameplay-cpu.md) | Time Profiler baseline and the first optimization candidates. |

`crates/skate-game/src/diagnostics/performance.rs` implements the
`SKATE_PERF_REPORT` harness used by most records.

The harness counts a hitch (`hitch_frames`, and `hitch` per sample) when a
frame is longer than twice the median of the 120 frames before it, upstream's
frame-time rule. Records before 2026-10-08 report `frames_over_8ms` instead, a
fixed threshold that most 120 Hz FIFO frames (about 8.33 ms) exceed: it flagged
85–88% of the frames in each of the eight FIFO runs behind
[2026-10-07-presentation-fifo](2026-10-07-presentation-fifo.md). Replayed over
the same samples, with the first 120 standing in for the warmup, the new rule
flags 1–4 frames per run. A frame that misses one vblank lands at about twice
the median, against a threshold of 16.5–16.8 ms, so only some count: 18 of the
55 frames between 12.5 and 20.8 ms in those runs. Longer stalls always count.
Windows of 30 to 240 frames change the FIFO counts by at most one.
