# Documentation

- [macOS guide](platform/macos.md): requirements, playing, controllers,
  original skating audio, local multiplayer and platform notes.
- [Audio research](audio/README.md): how the original game's audio works and
  what the port reproduces.
- [Performance records](performance/README.md): frame-time, GPU and CPU
  measurements.
- Rendering notes: [water animation](rendering/water-animation.md), the
  original PCA ocean table and the clock the water shaders read.
- Physics notes: [quarter-pipe transition input](physics/quarter-pipe-transition-input.md),
  [grind trick state 202](physics/grind-trick-state-202.md) and
  [solver iterations](physics/solver-iterations.md).
- World notes: [map starts and trigger-volume collision](world/spawns.md):
  where each map starts and why surfaceless collision boxes are skipped.

The Lua mod SDK is documented in [`sdk/`](../sdk/README.md). Other Markdown files placed
directly in `docs/` are ignored by Git and stay local.
