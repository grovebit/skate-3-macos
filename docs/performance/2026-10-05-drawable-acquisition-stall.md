# Focused gameplay slowdown, 2026-10-05

Continuation of task/hud-upload-cache, integration branch main at
8932a65aadb9eef90fb0b63224ec15655c78719a. No merge authorized.

Optimized executable SHA-256:
774fcdb32416241f819a8ed64c13e0dd0f3dbcf1893048e4b057eb5b3e94285e.
Process 85953. User confirmed game was focused and fully visible at onset.

## Capture

Native Metal listener plus Time Profiler attached to the game. CPU recording
18:28:57.596–18:29:33.961 UTC, stopped after user reported the drop. Requested
30-second rolling window with five-minute limit; exported samples actually span
0.667–36.362 seconds from recording start. Saved CPU trace is approximately
45 MB. Both profilers stopped cleanly. Raw trace, XML, game log and listener
output remain local in the task worktree; compact Metal intervals are in
2026-10-05-drawable-acquisition-stall.json.

## Observed drop

Times below are UTC; local Dublin time was one hour ahead.

| Interval start | FPS | Mean GPU ms | Mean drawable wait ms | CPU end-to-end ms |
| --- | ---: | ---: | ---: | ---: |
| 18:29:05.484 | 174.4 | 3.37 | 0.47 | 7.86 |
| 18:29:06.482 | 100.8 | 4.21 | 4.80 | 12.93 |
| 18:29:07.483 | 79.1 | 5.31 | 8.57 | 17.35 |
| 18:29:08.482 | 43.0 | 5.64 | 6.35 | 15.44 |

Native records then have a gap until 18:29:14.484 and sparse presentation
counts. These cannot establish that the game was entirely frozen; gameplay
state transitions continue in the game log. Do not interpret sparse interval
FPS as a complete per-frame trace. CPU end-to-end is latency including waits
and overlap, not exclusive computation. GPU means cannot exclude individual
GPU stalls; the 43 FPS interval has a maximum GPU duration of 10.59 ms.

CPU samples localize drawable acquisition to:

    bevy_render::view::window::prepare_windows
      wgpu::Surface::get_current_texture
        wgpu_hal::metal::Surface::acquire_texture
          -[CAMetalLayer nextDrawable]
            CAMetalLayerPrivateNextDrawableLocked
              usleep / nanosleep / __semwait_signal

The sleeping stack occurs at trace second 11.786663666, approximately
18:29:09.383 UTC. Another nextDrawable stack is present at trace second
8.677663458. Samples establish the call path, not total blocked wall time.

Summed sample weights per wall second fall from 3.21 before the dip
(18:29:04–06) to 1.89 during the dip (18:29:06–09.5), recovering to 3.25
(18:29:19–22). These include synchronization/syscall samples and are not a
precise utilization measurement. No corresponding burst of sampled CPU work
was found. Physics advance sample weight per wall second was essentially
unchanged (0.0445 vs 0.0446). Different gameplay intervals are not a controlled
benchmark.

## Interpretation and next check

Drawable acquisition backpressure is a concrete lead. This capture does not
establish its underlying cause: GPU scheduling, compositor delay, or queue
synchronization can all require further evidence. User confirmation excludes
switching apps or resizing as the reported onset trigger.

The game requests AutoNoVsync and continuous updates; stored graphics settings
have fps=0 (uncapped). Actual negotiated presentation mode was not captured.
Bevy 0.18.1 defaults desired maximum frame latency to 2. The installed wgpu-hal
27.0.4 Metal backend sets drawable count to configured maximum frame latency
plus one, disables nextDrawable timeout when supported, and calls nextDrawable
inside acquire_texture. These are source observations, not captured runtime
configuration.

Next targeted experiment: record actual presentation mode and drawable count,
then compare the same route under current presentation and FIFO with otherwise
identical settings. Measure drawable waits, frame-time tails and GPU duration.
This tests host presentation pacing without modifying gameplay logic. Do not
claim a fix or select a permanent mode from this capture alone.
