# Optimized HUD slowdown investigation

Continues `task/hud-upload-cache`, integration branch `main` at
`8932a65aadb9eef90fb0b63224ec15655c78719a`. Gameplay executable is the HUD
optimization at `c789d64`, SHA-256
`774fcdb32416241f819a8ed64c13e0dd0f3dbcf1893048e4b057eb5b3e94285e`.
No further runtime changes were made for this investigation.

## CPU capture

Instruments Time Profiler recorded 18:02:01.312–18:04:02.065 UTC on 2026-10-05,
target PID 76753. Duration 120.753 seconds; raw recording 64 MB. The user reports
a slowdown near the end, before closing the game. The capture has 256,501
nonempty weighted CPU stacks. Inclusive stack shares overlap and are not
frame-time percentages. Buffer creation appears in 2.532%, mod `snapshot_ro`
in 2.265%, player-physics snapshots in 1.769%, and mesh allocation/free in 0.074%.
The final five-second CPU bucket does not show a clear increase over several
earlier buckets. Reported thermal state stayed Nominal during this capture.
The CPU profile alone does not establish the cause of the user's FPS drop.

## Native Metal recovery

Installed Apple `metalperftrace(1)` documentation describes always-on Metal
frame timing and resource statistics. It can retrieve prior intervals without
running an Instruments Metal System Trace. Used:

```sh
metalperftrace collect --start 2026-10-05T18:01:40Z --end 2026-10-05T18:05:00Z --json native-metal-slowdown
metalperftrace overview --json-include-timeline --predicate 'pid == 76753' native-metal-slowdown/MetalPerfTrace_20261005_190140_to_190500.atrc
```

The complete native collection is 4.7 MB and covers the game through
18:04:11.960 UTC, including the CPU profiler's shutdown and later slowdown.
The JSON export's top-level process entries were filtered explicitly to the
game PID; the tool predicate can leave metadata entries with empty layers for
other processes. Only compact game timing metrics are committed here.
The game drawable changed from 3840x2102 to 3456x2168 at 18:02:45.964 UTC.
The last configuration persists until 18:04:06.964 UTC.

Selected approximately one-second native intervals:

| Interval start UTC | Reported FPS | GPU active average | CPU end-to-end latency average |
| --- | ---: | ---: | ---: |
| 18:03:59.964 | 158 | 5.66 ms | 10.60 ms |
| 18:04:00.964 | 75 | 5.89 ms | 16.81 ms |
| 18:04:01.964 | 15 | 4.77 ms | 59.26 ms |
| 18:04:02.964 | 27.1 | 4.90 ms | 35.30 ms |
| 18:04:07.964 | 67 | 4.94 ms | 17.32 ms |
| 18:04:08.964 | 67 | 4.92 ms | 17.44 ms |

GPU execution does not rise enough to explain the severe late drop. CPU-side
end-to-end latency and frame presentation interval increase instead. The CPU
metric is pipeline latency, **not measured CPU computation time**; it can
include waits and overlapping frame stages and must not be added to GPU time.

The steep drop coincides with the CPU profiler's recording limit and shutdown.
That is a correlation, not proof of profiler interference. A native-only session
with live GPU/frame timing and no Instruments recorder is the next useful test.
Do not assign this slowdown to HUD uploads, physics, thermal throttling, or a
specific CPU function on the available evidence.

Full raw CPU stacks, native trace, and richer native JSON remain local in the
task worktree. The compact committed timeline preserves all 140 populated
native intervals for inspection. Apple manuals are installed at
`/usr/share/man/man1/metalperftrace.1` and `MetalPerformanceHUD(1)`.
