# Hidden-board world query

Integration branch: `main`, starting commit
`37c3205f66d8b0aa888c2b7b6e1c1569d1b6e77d`.
Task branch: `task/upstream-small-fixes`. Source: the empty-volume hunk of
upstream draft PR #52 (head `54cf8f5`).

## Change

`BoardWorld::query_primitives` now returns after resetting its contact buffer
when it gets no volumes. Before, the union bounds of an empty list were `None`,
which `candidate_ranges` reads as "every triangle", so the query walked the
whole map with an empty inner loop. The early return leaves the same contacts
and buffer state: nothing is allocated, and `ContactBuffer::flush` returns at
once for an empty buffer. The test
`board_world::tests::empty_volume_query_matches_a_full_walk_that_finds_nothing`
compares the early return with a full walk over every triangle that finds
nothing, starting from the same used buffer. It passes with and without the
early return, and fails if the return is moved above the buffer reset. This is
host traversal only; no game behaviour changes.

## When it runs

`solve::advance` makes one board world query per fixed 60 Hz tick
(`physics/frame.rs`), after keeping only the volumes that board possession
enables. Possession disables all of them in two states:

- state 3, hidden: a released board more than `MaxDistance` from the player, or
  with board state 6. `physics_skatecontroller/default/MaxDistance` is
  `0x41F00000` (30.0) in `skatercollections.vlt`
  (SHA-256 `3b7dbd062bb1c906a085514355afff35cfa22f486ae70820c5ad1a42a7aab25b`);
- state 4, returning: from the recall request until the board reaches the hand.

A player who walks more than 30 m from the board therefore paid the full walk
on every tick until recalling it. The two other callers, the climbing path for
a dropped board and a test helper, pass the board's unfiltered volumes. The
skeleton query in the same solve has volumes in the riding and on-foot
collision modes; whenever its list is empty, it takes the same early return.

## Measurement

Apple M3 Max, shared with other build jobs. `test` profile (opt-level 3, the
same optimization as the `./play.sh` development build). The ignored test
`physics::map_startup::hidden_board_world_query_cost` loads a map's collision
with `collision_world`, then times 600 queries with no volumes and the game's
`query_settings`. Each map ran two or three times; ranges are shown.

| Map | Triangles | Before (µs per query) | After (µs per query) |
| --- | ---: | ---: | ---: |
| University | 1,133,649 | 930–1,285 | 0.01–0.02 |
| DownTown | 1,545,276 | 1,280–1,290 | 0.01 |
| SkateSchool | 91,790 | 69–83 | 0.01 |

Map SHA-256: University
`b7f6379815efafd095b15f50504b920365d5d119692657951779e93abade46b9`, DownTown
`362eb7b99057c89d53ebc37c7bb011a5bb2e16bb57fa1e46a304dfcea8bbfc21`, SkateSchool
`89ba92c6db28f48cb1da5c21e0d14d7766caa2da571ca73a54d5209785dbcc87`.

On the large maps the saving is about 1 ms of CPU per tick while the board is
hidden or returning. This times the query alone; no frame-time change is
claimed. Upstream reported 27–29 ms per tick for the same path in its own build;
that figure was not reproduced here.

Reproduce with `SKATE_MAP_TEST_PATH=<installation>/maps/University.skate cargo
test --offline -p skate-game --bin skate3rust hidden_board_world_query_cost --
--ignored --nocapture`.
