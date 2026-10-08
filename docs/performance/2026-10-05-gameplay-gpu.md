# University short Metal GPU baseline, 2026-10-05

Integration branch: `main`, starting commit
`dc171c33d5d95c1f34ce8fb423f07fc3ed402bf4`.
Task branch: `task/performance-profile`.
Same optimized development executable and University setup as the CPU baseline.
No runtime changes were made.

## Capture and method

Instruments 27 Metal System Trace attached to gameplay child PID 67214 on
Apple M3 Max. Five-second recording limit; the recorder reports 6.153314 seconds
including its start/stop tail (17:44:18.426 to 17:44:24.579 UTC).
Game GPU intervals span 5.899200 seconds. The completed raw trace is about
2.1 GB; finalization briefly used approximately 12 GB RSS before dropping to
3 GB. This is an instrumented, short interactive sample, not an uninstrumented
frame-rate benchmark. The trace captures only part of the user's play session.

An earlier three-minute trace generated 61 GB of raw data and exited with
status 137 during finalization. It has no readable final document and provides
no reliable timing results. It remains local, as does the completed short trace.

Local artifacts are in `/private/tmp/skate-3-performance-profile/`:
`gameplay-metal-short.trace`, `game-gpu-short.log`, `metal-short-toc.xml`, and
four XML exports for GPU intervals, command-buffer frame assignment/completion,
and presented handlers. Raw captures may include process environment metadata.

Resolve Instruments XML references, filter `metal-gpu-intervals` to game PID
67214 and nesting depth zero, and join command buffer IDs against
`metal-command-buffer-frame-assignment`. For each frame, merge overlapping GPU
intervals across Vertex, Fragment, and Compute channels. This avoids summing
parallel or nested execution as if it were serial GPU time. Drop the first and
last mapped frame to reduce capture-boundary effects, leaving 753 frames.
Seventeen game intervals had no command-buffer frame mapping and were omitted
from per-frame statistics; all identified game intervals enter the window-wide
activity calculation. Quantiles use the sorted element at floor((N-1)*0.95).
The committed JSON contains the derived measurements.

## Results

| Metric | Mean | Median | 95th percentile | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Union of GPU active intervals per frame | 3.86 ms | 3.28 ms | 7.53 ms | 9.96 ms |
| First GPU start to last GPU end per frame, including gaps | 4.11 ms | 3.55 ms | 8.03 ms | 10.45 ms |
| Presented-handler callback spacing | 7.82 ms | 6.36 ms | 16.72 ms | 61.35 ms |
| CPU-to-GPU latency per execution interval | 2.19 ms | 1.68 ms | 5.31 ms | 10.20 ms |

The union of game GPU active intervals covers 49.38% of the measured GPU window.
This measures traced game activity, not a hardware utilization counter or all
applications' GPU use. Per-channel active time is 1706.68 ms Vertex, 2250.14 ms
Fragment, and 129.94 ms Compute; channels overlap and must not be added.
Presented-handler spacing is callback cadence, not a verified physical-display
FPS measurement. CPU-to-GPU latency is per execution interval, not whole-frame
latency or input latency.

The largest labeled GPU pass by summed interval durations is
`main_opaque_pass_3d` (1605.32 ms across the sample), followed by upscaling
(410.79 ms), transparent 3D (400.49 ms), UI (294.95 ms), and retail exposed tone
(273.96 ms). These labeled sums overlap across channels and are not exclusive
wall-time shares.

## Interpretation

This short sample does not show a continuously saturated game GPU. The active
GPU time per frame is below the 16.67 ms budget for 60 Hz throughout the mapped
interior sample, while callback gaps sometimes exceed that budget. Investigate
CPU submission, scheduling, resource churn, and presentation pacing before
reducing visual fidelity. This is a direction for further measurement, not
proof of a single bottleneck; tracing overhead and gameplay variation matter.
The CPU baseline's repeated HUD asset preparation remains a concrete candidate.
No original physics, APT animation, lighting, or gameplay timing was changed.
