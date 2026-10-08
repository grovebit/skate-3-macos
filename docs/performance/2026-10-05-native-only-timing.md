# Native-only gameplay timing

Continues `task/hud-upload-cache`, integration `main` at
`8932a65aadb9eef90fb0b63224ec15655c78719a`. The executable remains the optimized
HUD build (`c789d64`), SHA-256
`774fcdb32416241f819a8ed64c13e0dd0f3dbcf1893048e4b057eb5b3e94285e`.
No further game code changes or merges have occurred.

## Collection

Launch with `MTL_HUD_ENABLED=1`; use the installed Apple native tool:

```sh
metalperftrace listen --json --pid 81541 --interval 1 --output native-hud-live.json
```

Stopped the listener with SIGTERM after the user finished. It cleanly finalized
its JSON array. No Instruments CPU or GPU recorder ran during this session.
The raw listener output is about 1.2 MB. Captured 243 intervals, 41,709 presented
frames, and 242.862 seconds, from 18:13:50.910 to 18:17:53.773 UTC on 2026-10-05.
Overall presented throughput is about 171.7 FPS across mixed gameplay.

Recovered native configuration data separately with `metalperftrace collect`.
Drawable size is 3456x2168 from 18:13:43.909 to 18:17:50.908 UTC, covering both
comparison windows and the early dip. Resolution changes therefore do not
explain these particular comparisons. Only game timing rows are committed;
full native logs and trace remain local in the task worktree.

## Measurements

The comparison windows below are chosen for describing the recording, not a
controlled scene or route. FPS is frame count / active duration. Other means
are weighted by presented frame count.

| Window UTC | Reported FPS | GPU active time/frame | CPU end-to-end latency | Drawable wait |
| --- | ---: | ---: | ---: | ---: |
| 18:13:55–18:14:00 | 205.9 | 2.84 ms | 6.76 ms | 0.52 ms |
| 18:16:50–18:17:20 | 115.3 | 4.82 ms | 11.91 ms | 0.51 ms |
| 18:17:40–18:17:50 | 128.0 | 4.30 ms | 10.60 ms | 0.44 ms |

The worst populated approximately one-second interval is 18:14:06.911–18:14:07.911:
72.0 FPS, 6.98 ms average GPU active time, 19.69 ms CPU-side end-to-end latency,
and 9.28 ms average drawable wait. The preceding interval reports 84.8 FPS with
4.43 ms GPU active time and 7.04 ms drawable wait.

All 243 intervals report 78 command buffers per presented frame. No new shader
compilations are reported during these intervals. Individual GPU times spike
higher than their interval averages (e.g. 25.10 ms maximum in a late interval);
mean GPU duration alone does not exclude occasional GPU-related hitches.

## Interpretation and next work

Slow intervals persist without the Instruments recorder, so its shutdown cannot
explain all observed drops. Both GPU execution duration and CPU-side pipeline
latency rise in the later, slower window. This is not enough to name a single
CPU function or prove that the GPU never limits an individual frame. Native
CPU end-to-end time is latency, not exclusive CPU computation time, and includes
pipeline waits. GPU and CPU means must not be summed as serial frame costs.

The early sharp dip has substantial drawable acquisition wait: inspect
presentation/queue behavior. For the sustained late decline, inspect command
submission/resource churn and obtain a short CPU sample while FPS is already
low. The 78-command-buffer count is a concrete investigation target, not proof
that reducing it is correct or will improve performance. Keep original gameplay,
APT frame advancement, lighting, and ordering intact; do not tune those to hide
unidentified stalls.

Earlier CPU profiles show mod snapshot serialization, buffer creation, and
renderer scheduling as candidates. This native session does not attribute the
late decline to any one of them. Gameplay/camera changes, OS scheduling, clock
changes, and other competing work remain uncontrolled. A native overlay may
also add overhead; this was not a completely uninstrumented run.
