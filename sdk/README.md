# Lua mod SDK (API 2)

Mods are Lua packages with a `mod.json` manifest (`"api": 2`) and an entry Lua
file that returns a table of callbacks. They build gameplay from low-level
physics and graphics primitives; there are no host-side vehicle or wheel
classes.

## Reference

- [`GENERAL_API.md`](GENERAL_API.md): reading game systems, commands and
  results, native bodies and constraints, contacts, input and graph control.
- [`ENGINE_API.md`](ENGINE_API.md): engine observations and controls.
- [`DEFORMATION.md`](DEFORMATION.md): generic impact deformation.
- [`skate.lua`](skate.lua): language-server declarations for editor completion
  (not executed).

## Examples

| Example | Shows |
| --- | --- |
| [`physics-sandbox`](examples/physics-sandbox/) | The physics/graphics layer: spawn a dynamic box, push it, attach to it. |
| [`skyline`](examples/skyline/) | A GLB rigid-body car with suspension, steering hinges, axle motors and tire contact. |
| [`broken-bones`](examples/broken-bones/) | Impact-based limb injuries from native contacts and joint topology. |
| [`wipeout-challenge`](examples/wipeout-challenge/) | Platform survival with moving hazards and water sensors. |
| [`game-of-skate`](examples/game-of-skate/) | Host-first sequential turns in multiplayer. |
| [`simon-says`](examples/simon-says/) | Copying landed tricks, with host authority and lives. |

## Make a mod

Copy an example, edit `mod.json` and `main.lua`, then validate the package:

```sh
cargo run --locked -p skate-mods --example check_mod -- path/to/your-mod
```

Put the folder (or a `.zip` of it) in [`mods/`](../mods/README.md) to load it
with `./play.sh`. The host side lives in `crates/skate-mods/` (Lua runtime and
schema) and `crates/skate-game/src/modding/` (game integration).
