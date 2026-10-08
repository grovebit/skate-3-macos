# Solver iterations

The board and the skater share one `rw::physics::Simulation`. Its word at
+0xB0 (176) is the number of passes the constraint solver makes over the
contact, joint and drive rows in each step. The port solves with 50, the value
the original stores before every offline gameplay step. It used to read
`physics/default.RWMaxIterations` (25), which the original never reads.

Traced on 2026-10-08 in the base-disc executable: `default.xex` SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`, mapped
`default.pe` SHA-256
`ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42` (base
`0x82000000`). Addresses are base-disc unless labelled TU3.

## Where the count comes from

- The Simulation constructor `0x82ABC5C0` (TU3 `82AE4A30`) defaults +0xB0 to
  50 (`li r3,0x32`, stored at `0x82ABC91C`) and +0xA8 to 30.
- World setup `0x8273B170` builds the Simulation config with a hard-coded 25
  (`li r27,0x19` at `0x8273B1C4`, config+0x10). The wrapper constructor
  `0x82D97918` copies it to +0xB0 (`0x82D97E8C..0x82D97E90`), so a new world
  holds 25.
- The frame update `0x82834E00` runs the begin-step `0x82D98090` on each active
  owner's Simulation wrapper, then the writer `0x827412C8` for that owner
  (calls at `0x82835164` for slot 0 and `0x82835174` for slot 1). The writer
  runs on every simulated frame, not only when the mode changes:
  - `lwz r11,8(r3)` reads the owner's mode word and `0x827412E4..0x827412F4`
    reduce it to (mode != 3);
  - `li r10,0x32` (50) at `0x827412F0` is replaced by `li r10,0x19` (25) at
    `0x82741308` when mode != 3;
  - `stw r10,0xB0(r9)` at `0x8274130C` stores it in the owner's Simulation
    (`**(owner+0xC)`).
- The solver reads the count: thunk `0x82D98180` loads +0xB0 into r9 and
  tail-calls `0x82ABA360` (TU3 `82AE27D0`, ported in
  `crates/skate-core/src/physics/solver.rs`). `0x82ABAC80` (TU3 `82AE30F0`,
  thunk `0x82D98170`) reads it at `0x82ABAC8C`. Both loop until the count is
  used up (back edges at `0x82ABAC78` and `0x82ABB554`); there is no
  convergence exit.
- Neither thunk is called directly. `0x82D98390` stores their addresses
  (`0x82D984BC`, `0x82D985FC`) in stage descriptors of the Simulation
  wrapper's pipeline. After both writer calls, the frame update
  steps each owner with `0x827413A8` (its only callers, `0x82835228` and
  `0x8283523C`), which dispatches the owner's pipeline jobs through
  `0x82D96968`, so those pipelines run after the count is written. That the
  solver stage is among those jobs was not followed further; the TU3 traces
  below show that no solve used 25.

## Which mode gameplay uses

- The owner constructor `0x82740770` stores its mode argument at owner+8
  (`stw r5,8(r3)` at `0x82740788`). The only other store is set-mode
  `0x82741C50` (`stw r4,8(r3)` at `0x82741C60`).
- Setup `0x8273B170` copies the global byte `0x83026AE9` to setup+0x10 (`lbz`
  at `0x8273B22C`, `stb` at `0x8273B240`) and constructs two owners:

  | Owner | Simulation | Byte 0 | Byte not 0 |
  | --- | --- | --- | --- |
  | slot 0, setup+0x8 | wrapper at setup+0x24 | mode 3: 50 passes | mode 1: 25 passes |
  | slot 1, setup+0xC | wrapper at setup+0x28 | mode 0: 25 passes | mode 2: 25 passes |

- `0x8273C480` re-applies the byte when a session starts: if setup+0x10
  differs from the byte, it copies the byte and calls set-mode with (1, 2) for
  a set byte or (3, 0) for a clear one. It is called from
  GameModule::OneOffLoad (`0x826BA65C`) and from the world session start
  `0x82832F88` (`0x82833044`).
- Both writer calls sit behind world+0x91. Slot 0's writer runs when the
  driver's step flag (bit 0) is set. Slot 1's needs bit 1 as well as a
  non-null setup+0xC and a non-zero setup+0x10 (`0x828350DC..0x82835174`), so
  offline it never runs.
- The byte is zero at load (BSS). `.text` writes it in two places, both in the
  online flow:
  - `0x826C7464` stores 1 in `0x826C73A0`, whose only caller is the online
    session event handler `0x826C4020` (`0x826C4404`). `0x826C73A0` also sets
    0x8305BC38+0x143, which makes the network selector `0x82837F08` create
    `Net::OnlineNetwork` instead of `Net::OfflineNetwork`.
  - `0x826BBFFC` stores 0 in `0x826BBF60`, a reset that then starts a new
    session. Its callers, `0x826C3A64` and `0x826C70F8`, are online-state
    functions.

So only EA's online session flow sets the byte. Offline play runs slot 0 in
mode 3, and every step solves with 50 passes.

Native replays of `0x827412C8` (modes 0, 1, 2, 3, 4, 0x80000003 and
0xFFFFFFFF), `0x82741C50`, `0x8273C480` (cached and global byte 0, 1 and 0xFF)
and the setup's byte copy and constructor arguments in our PPC interpreter
matched this reading. The replay script is local
(`.local/upstream-port/scripts/verify_iteration_mode.py` in the task worktree).

## RWMaxIterations is not read

`RWMaxIterations` is the Int32 layout field at +0x28 of the `physics` class
(key `0x04293D0F2ADCFBB4`). The only bound instance, physics/default, is held
by the owner at +0x1C, with its layout pointer at +0x20 (`0x82740370`). The
reads through that pointer take only WorldGravity (+0x00): the owner
constructor at `0x827409F0..0x82740A10` (into Simulation+0x90) and the
live-edit apply `0x82749148`. Nothing reads +0x28, and the field's key is not
looked up anywhere. Its stock value, 25, equals the setup constant, but the
setup uses its own `li`.

## Port

`crates/skate-game/src/physics/settings.rs` sets `SIMULATION_ITERATIONS = 50`
for the board/skater solve (`BoardStepSettings::iterations`). The read of
`RWMaxIterations` and its zero check are gone. The port has a single skater
Simulation, slot 0, and nothing corresponds to the online byte.

The multiplayer extension is a transport-neutral free-skate, not EA's online
session flow, so it also solves with 50. This is a host choice. In the
original, an online session sets the byte, and both owners solve with 25.

## Measurements

Our build on 2026-10-08: Apple M3 Max, the development profile that
`./play.sh` builds (`opt-level=3` with debug assertions).

### Behaviour

The ignored tests in `crates/skate-game/src/tests/solver_iterations.rs` run the
production tick (`frame::advance`) on the stock skater. Each run first lets the
spawn drop settle for 120 ticks (Ground, four wheel contacts).

```sh
SKATE3_ASSET_ROOT=<installation>/assets cargo test -p skate-game --bin skate3rust -- --ignored --nocapture solver_iteration_measurements
```

| Measurement | 25 passes | 50 passes | TU3 (upstream doc 12) |
| --- | ---: | ---: | --- |
| Mean wheel vertical velocity at rest, 8 s | −0.0103 m/s | −0.00093 m/s | −0.012 m/s with 25; retail −0.001 m/s |
| Standing ollie: deck above rest at the pop's 4th frame | 6.62 cm | 7.94 cm | 25 about 1 cm low by the 4th frame |

- The resting velocity is a residual of each step. The wheels' height does not
  drift over the 8 seconds with either count. With 50 the deck rests 0.45 mm
  higher.
- The ollie leaves Ground on the same tick with both counts. With 25 the deck is
  0.88 cm low at the 3rd frame and 1.31 cm low at the 4th; the apex (1.160 m
  above rest) is the same.

With 50 the port matches the TU3 retail resting velocity; with 25 it shows the
error upstream measured.

### Cost

In-process (`tick_cost`): eight alternating rounds, each a fresh run that rests
on the board for four seconds and then bails from the raw bail chord for three
seconds. Milliseconds per 60 Hz tick (median of rounds):

| Window | Contacts | 25 passes | 50 passes | Collision and solve span |
| --- | ---: | ---: | ---: | --- |
| At rest on the board | 22 | 0.223 | 0.299 | 0.140 → 0.211 |
| Bail (ragdoll) | up to 101 | 0.353 | 0.515 | 0.280 → 0.442 |

All of the difference is in the collision-and-solve span; animation graphs and
the post-solve skater work cost the same. Earlier runs on the same day measured
0.212 and 0.273 ms at rest, so the absolute values move with machine load.

In game (`SKATE_PERF_REPORT`, University, saved settings, 120 Hz built-in
display with FIFO presentation): eight alternating rounds of one executable,
idle at the spawn. Medians, with ranges:

| Metric | 25 passes (7 runs) | 50 passes (8 runs) |
| --- | ---: | ---: |
| FPS | 118.9 (114.4–119.7) | 119.4 (118.8–119.9) |
| Frame-time SD | 1.03 ms | 0.79 ms |
| 99th-percentile frame | 16.0 ms (9.3–17.3) | 9.5 ms (9.2–16.0) |
| Main schedule | 2.22 ms | 2.23 ms |
| Physics, frames with a tick | 0.548 ms (0.539–0.552) | 0.606 ms (0.599–0.610) |

The extra passes cost 0.06–0.08 ms per tick at rest and 0.16 ms in a bail,
and nothing measurable in frame pacing: presentation still bounds the frame,
and the run-to-run spread is larger than any difference between the counts.

Method notes:

- The machine was shared with other builds (load average 7–14) and switched
  between AC and battery power during both groups.
- The measurement executable was a temporary build, not committed: it read the
  count from an environment variable, so both counts ran the same binary, and
  it fed neutral controller input. A connected controller had driven the skater
  in earlier runs. Two separately built executables also differed by up to
  0.5 ms of main-schedule time in frames without a physics tick, in opposite
  directions for two build pairs.
- One 25-pass run paused itself after five seconds (Escape reached the window)
  and is excluded.

## TU3 corroboration

Upstream's research (`docs/hails-additions/12-solver-iterations.md`,
skate3recomp traces of Title Update 3) is a reference only:

- TU3 writer `82763E00` (frame update `82859E70`), owner constructor
  `827632A8`, setup `8275DCC8`, setup copy `82DC2840` and mode byte
  `0x83082929`. They match the base-disc functions above by code shape and
  constants; the addresses are not interchangeable.
- ITERSET/ITERTICK hooks in Super-Ultra Mega-Park: only slot 0's writer ran,
  about 60 times a second, always in mode 3. The count went from 25 to 50 on
  the first gameplay frame and stayed 50 through riding, bails on and off the
  board, walking, respawns, the pause menu, Instant Replay and the Replay
  Editor. Menus and replays stop the frame update, so nothing is written or
  solved there. The solver logged only 50, and slot 1 stayed in mode 0 without
  being written.
- Those traces did not cover Party Play or online play.
- Upstream reads the mode as chosen once, when the world is built. The base
  disc also re-applies the byte at each session start (`0x8273C480`). Both
  readings give mode 3 offline.

## Open questions

- The original solves with 25 in online sessions. The port's multiplayer uses
  50 (host choice above).
- The base-disc evidence is static and replayed; the byte has not been traced
  at run time on the base disc. The TU3 traces agree with it.
