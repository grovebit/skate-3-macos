# HUD upload optimization

Integration branch: `main`, starting commit
`8932a65aadb9eef90fb0b63224ec15655c78719a`.
Task branch: `task/hud-upload-cache`.

## Scope and implementation

This changes the Bevy asset upload adapter in `scoring_hud.rs`. The original
APT VM, fixed-step advancement, scoring inputs, movie traversal, slot draw
order, visibility, map reset, and render-target resize remain unchanged.
Every frame still generates the same authoritative APT draws.

Compare positions and UVs against the existing owned mesh before constructing
replacement buffers or requesting mutable asset access. Compare color multiply,
color add, and atlas handle separately before modifying the material. No
approximate thresholds, hashes, secondary geometry cache, or animation skips
are introduced. Slot-owned topology and normals are fixed by construction.
The same coordinate-conversion helper serves comparisons and mesh creation.

This removes three per-slot vertex-vector allocations and downstream mesh
preparation for unchanged geometry, as well as bind-group preparation for
unchanged materials. Changed draws still update immediately. Array comparisons
add linear, allocation-free CPU work, which should be evaluated against the
avoided allocations/uploads in frame-time measurements.

## Evidence and validation

The original data producer is the owned
`data/fe/source/screens/hud2/trickdisplay2` APT bundle. Installed provenance:

- APT SHA-256: `57370fe8901e92c7d624ecac0c3ec8dcb25741d0b94eb00d07fadb61829c9c22`.
- Constants SHA-256: `788cb9a5782da8f6b4b9e723d76f92f2f63db4b0c219227b3a2c20609aa56ec4`.
- Collections SHA-256: `18c9b4251ef5e769b65b16b8ed62b533ab439495c6f96a60cb7690bc8692ab47`.

The host pipeline is scoring HUD input -> original runtime update -> retained
APT draw traversal -> position/UV/color/texture data -> Bevy mesh/material assets
-> renderer preparation. Only the final host asset-update step changes. This
task makes no new claim of complete original-game HUD behavioral parity and
uses no recovered executable constants on new host quantities.

Three focused tests passed, including the explicitly enabled owned-data replay:

- 500 unchanged draw updates emit zero mesh/material `Modified` events.
- Position, UV, vertex count, multiply color, additive color, and atlas changes
  independently invalidate the relevant asset; empty geometry and subsequent
  reuse also update correctly.
- Existing target-resize/composite-refresh coverage continues to pass.
- A 600-frame original APT runtime replay includes trick, multiplier/stance,
  and score-close inputs. Every uploaded position, UV, normal, color, and atlas
  matches the previous unconditional construction path. Existing slots would
  have produced 6,511 mesh modifications and 6,511 material modifications;
  the new path produces 326 mesh modifications (95.0% fewer) and 374 material
  modifications (94.3% fewer). New-slot allocations are excluded equally.

These are measured asset-event reductions, not an FPS claim. The replay checks
host output equivalence to the prior adapter; it is not an original-game trace.
It does not replace interactive visual checks across all gameplay states.

Reproduce with `cargo test --offline --locked -p skate-game --bin skate3rust
scoring_hud:: -- --include-ignored --nocapture`, setting `SKATE_HUD_TEST_SOURCE`
to the installed `private/hud/runtime/trickdisplay.json`. Ordinary test runs
skip only the owned-data test. Formatting, diff whitespace checks, and an
optimized development build also passed.

## Live render checks and frame-time limitation

Both original and optimized executables successfully rendered University and
completed the existing automatic performance harness (10-second warmup,
15-second sample, no GPU queries or serialized render-phase instrumentation).
The baseline observed 84.4 FPS / 11.85 ms mean frame time and the optimized run
174.7 FPS / 5.72 ms. These numbers are **not a valid controlled comparison**:
baseline logs remain idle on the ground, while the optimized session contains
tricks, bails, and teleports. No FPS gain is attributed to this change. A repeated
matched gameplay route, or guaranteed idle input, is still needed for that claim.
Draw statistics are zero because the optional draw instrumentation was not
active; these fields do not mean the scene was empty.

Compact reports are committed in `2026-10-05-hud-upload-frame-times.json`; full sample
arrays and run logs remain local in the task worktree. Executable SHA-256:

- Baseline: `ab09ac7f107552e855f9ba8238570d090565f248c126d3d8c8b0377c7d146bb1`.
- Optimized: `774fcdb32416241f819a8ed64c13e0dd0f3dbcf1893048e4b057eb5b3e94285e`.

The simplify review found no correctness or efficiency issues. Its reuse finding
was applied by sharing the HUD position conversion. Existing per-frame visibility
commands remain outside this upload optimization; changing those requires
separate state handling and validation.
