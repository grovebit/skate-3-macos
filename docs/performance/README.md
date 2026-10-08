# Performance records

Measurements on an Apple M3 Max, newest first. Each record names its build,
method and limits; JSON files hold the compact data behind a record. Commit
hashes in records dated before 2026-10-07 refer to the history before the fork
was squashed, kept locally on the `backup/pre-squash-6a9c7db` branch.

| Record | Summary |
| --- | --- |
| [2026-10-07-presentation-fifo](2026-10-07-presentation-fifo.md) | Frames waited on drawable acquisition; FIFO vsync gives a steady 119.7 FPS instead of 78 FPS with uneven pacing. |
| [2026-10-05-native-only-timing](2026-10-05-native-only-timing.md) | Four minutes of native Metal timing without Instruments; slow intervals persist without a profiler attached. |
| [2026-10-05-drawable-acquisition-stall](2026-10-05-drawable-acquisition-stall.md) | A focused slowdown traced to `nextDrawable` waits rather than CPU work. |
| [2026-10-05-slowdown-native-metal](2026-10-05-slowdown-native-metal.md) | A late slowdown recovered from always-on Metal statistics. |
| [2026-10-05-hud-upload-optimization](2026-10-05-hud-upload-optimization.md) | Unchanged scoring HUD meshes and materials are no longer re-uploaded. |
| [2026-10-05-gameplay-gpu](2026-10-05-gameplay-gpu.md) | Short Metal System Trace baseline: GPU active about 3.9 ms per frame. |
| [2026-10-05-gameplay-cpu](2026-10-05-gameplay-cpu.md) | Time Profiler baseline and the first optimization candidates. |

`crates/skate-game/src/diagnostics/performance.rs` implements the
`SKATE_PERF_REPORT` harness used by most records.
