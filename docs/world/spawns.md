# Map starts and trigger-volume collision

Each converted `.skate` map stores one start: a position and a heading. The
game places the skater there whenever the map loads, at launch or from the
map menu. `tools/asset_pipeline/map_starts.py` takes that start from the
original game's data. It replaced a host rule that placed the skater 1 m
above the upward collision triangle nearest the map origin, with a fixed
point at (330, −710) for University and heading 0 everywhere.

The start of SkateSchool lies inside invisible boxes that the loader used to
make solid, so the collision loader now skips them. That rule is empirical and
provisional; see [Surfaceless collision meshes](#surfaceless-collision-meshes-provisional).

## Evidence base

- Executable: base-disc `default.xex`, SHA-256
  `1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`, read
  as the mapped image `default.pe` (SHA-256
  `ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42`, base
  `0x82000000`). All addresses below are base-disc addresses, found by
  disassembly. Upstream's TU3 research was used only as a lead.
- Data: `data/big/db.big` (`skatercollections` `world` and `fe_locations`
  rows), `data/content/global_locators/**/*.rx2` (EB0009 records) and the
  `cSim_*.xsf` simulation streams of each `worldDIST_*.big`.
- Field keys are 64-bit VLT hashes. The executable builds them as immediates
  (`lis`/`ori`/`rldimi`), which is how the readers below were found.

## How the original chooses the start

The session keeps the start in a global structure at `0x8305BC38`: the world
row key at `+0x30` and the start-locator name at `+0x40` (`0x8305BC78`).

**Writers of the name.**

- `826C77E0` (called from `826C73A0`) resolves the session's world row. If
  the world's bool `Hash_0C54AD24155E3220` is set, it copies the world's text
  `Hash_3735C5C12E8E7AE1` into `+0x40`. The bool is set in
  `default_skateparks` (inherited by every park world) and is called the park
  flag here. With an active challenge (class `challenges`,
  `0x8876E0B556740E36`) on a non-park world it copies the challenge target's
  `Location` text (`0xF1C6D5FA4A23951E`) instead. A non-park world without a
  challenge keeps whatever name was set before.
- `825F1600` (virtual; slots `0x822FDCD0`, `0x82339DD0`) builds the
  `fe_locations` (`0xDDCD6BC23255E508`) row `gamestart`
  (`0x2E33DBE1D6B33472`) when its third argument is non-zero, otherwise
  `schoolstart` (`0x2E02DBADFCF1FDF8`). It copies the row's `location` text
  to `+0x40`, its `World` key to `+0x30`, and posts a world-load message. The
  meaning of the argument was not traced.
- `826BAFC8` falls back to `gamestart` (location and world) when the requested
  world key has no `world` collection. `826E4050` also writes `gamestart`.
- `8283F758` (virtual; slots `0x8230F97C`, `0x82347A78`) copies any name and
  world key. Its callers were not traced; map-menu travel is the likely one.
- `826FBE38` clears the name.

**Locator lookup.** `8287C568` passes `data\content\global_locators` to the
`*.rx2` loader `828E8B68`. The locator manager is a `0xD140`-byte object
(constructor `828E90D0`, vtable `0x82310F90`) that `826B57D0` stores at
`+0xA8` of the object it registers at `0x83027D34`. `828E9840` hashes a name
with `82946F78` (case-sensitive: `h = (h << 4) + c`, top nibble folded back
with a 23-bit shift) and finds the record in an ordered tree (`828E94C0`). It
returns a pointer to the record, whose first 64 bytes are its 4x4 matrix, or
to a default matrix inside the manager (`+0xD0E0`) for an unknown name;
`828E97E0` returns the translation row.

**EB0009 layout.** The type's pointer fix-up pair (`8293AE48` turns pointers
back into offsets) gives the layout. A section starts with the record count
(`+0`), the spawn-slot count (`+4`), and the offsets of the records (`+0xC`),
the slot array (`+0x10`) and the names (`+0x14`). A record is 128 bytes: a
row-major matrix at `+0` (row 2 forward, row 3 position), its slot count at
`+0x60`, and section-relative pointers to its first slot (`+0x64`), its name
(`+0x68`, `teleports.location_records`) and its group name (`+0x6C`). A spawn
slot is `0x70` bytes: a matrix, then pointers to its name (`+0x60`) and group
(`+0x64`). In the shipped `global_locators` (23 sections, 1,039 records,
2,155 slots) every slot pointer lies in its section's slot array, the counts
add up to the section header, and every slot is named `parent::child`, mostly
`…spawnlocator…` or `…spawn…`. 431 records own slots; 339 of them own six.

**Consumers.**

- `826E8660` and `826BB190` look up the start name's position and pass it to
  world streaming (`8287C118`, `8287BEF8`).
- `826B8AC8`, called at the end of `826BB190`, builds a per-player spawn
  object (constructor `828700E8`, or `828737D8`, which calls it, when the
  session word at `0x8305BC38` is 2). Both pass the locator manager as the
  resolver (`+0x10`) and a player index (`+4`). The index is 0 unless the
  byte at `0x8305BC38+0x143` is set and the byte at `+0x140` is clear; then
  it is the local player's place among six session entries (stride `0xA8`).
- `82872D80` arms the spawn object. If its caller passes no locator hash and
  `8286FFF0` finds no object whose transform it accepts, it stores the
  `82946F78` hash of the start name (`+0xD8`). On a later update
  (`82871278`) `82872F00` calls the resolver's virtual slot 1, `828E9598`,
  with that hash and the player index, and passes the resulting 4x4 matrix to
  the owner's `+0x14` object (virtual slot 8, not traced), then posts an
  event. Orientation is part of that matrix.
- `828E9598` finds the record (`828E94C0`) and `828E9688` builds the matrix
  for player index `i`. A record with slots gives slot `i`'s matrix when
  `0 <= i < count`. Otherwise it takes slot 0's matrix, or the record's own
  matrix when the record has no slots, and moves row 3 by `(0, 0, −5i)`
  (constants `0x820BAF10` = −5.0 and `0x82163B30` = 0.0), which is zero for
  player 0. A missing record gives a default matrix built from `i + 1`.

So a park always starts at its world row's start locator, and player 0
stands at that locator's first spawn slot when it has one. A park's
`fe_locations` rows do not decide the start: MegaPark's selectable row names
`vert_skp5_01_challengelocator_01`, which exists nowhere in the shipped
content.

## What the pipeline bakes

`map_starts.start(game_root, converted, district)` resolves one district:

1. **Park**: if the district's root world row (the row of that `WorldStream`
   whose parent is not another row of the stream) has the park flag, its
   `Hash_3735C5C12E8E7AE1` locator. This is the `826C77E0` selector. Mode
   variants (full, empty, heatmap, tutorials) inherit the root row's locator;
   only `dist_startpark_tutorialsignup` names another one
   (`freeskate_startpark_locator`). Using the root row is a host choice for a
   standalone map.
2. **Chosen row**: `DEFAULT_LOCATIONS` for districts with several
   `fe_locations` locations. University's `gamestart` is the executable's own
   new-game and fallback start. Industrial, DownTown and MaloofMoneyCup are
   project choices, labelled provisional: retail picks a location per session.
3. **Only row**: the district's single `fe_locations` location (BlackBoxPark;
   SkateSchool, whose two rows `dist_skateschool` and `schoolstart` name the
   same locator). For BlackBoxPark this assumes arrival through its only
   destination and is provisional.

The placement is player 0's (`828E9688` with index 0): the record's first
spawn slot when it owns slots, otherwise the record itself
(`map_starts.first_slot`). Of the ten starts only SkateSchool's and
University's records own a slot, one each. The slots are 3.0 m (SkateSchool)
and 5.0 m (University) from their records, and University's faces the
opposite way.

The locator must have exactly one distinct placement among the district's
own `global_locators/*/<District>/*.rx2` files. A district without a start, or
with a missing or ambiguous locator, fails its map conversion with the reason;
there is no geometric fallback. Each of the ten chosen names has exactly one
record in the whole `global_locators` tree, so the per-district directory
model does not change any result.

The position is row 3 of the placement matrix. The heading is
`atan2(row2.x, row2.z)`, written into the header float after the spawn. The
game builds the deck basis with `Mat3::from_rotation_y(heading)`
(`crates/skate-game/src/physics.rs`), whose forward column
`(sin h, 0, cos h)` is the placement's horizontal forward. That matches the
teleport path, which applies a locator matrix's rows as the deck basis. The
header keeps yaw only; the up row of all ten placements is `(0, 1, 0)`
within `2e-7`.

| Map | Source | Placement (EB0009 record or spawn slot) | Start | Heading | Label | Previous spawn |
| --- | --- | --- | --- | --- | --- | --- |
| BlackBoxPark | `fe_locations dist_blackboxpark` | `A_BlackBoxPark` (`BAM/DIST_BlackBoxPark/DIST_BlackBo_[0x5d330daf3d22bc79].rx2+0x250`) | (10.55, 0.08, 6.05) | 90° | only row, provisional | (−1.81, 1.00, −1.30) |
| DownTown | `fe_locations dist_downtown_skatepark` | `Z_DT_SkatePark` (`BAM/DIST_DownTown/DIST_DownTow_[0x8927ff0fa1ae5f27].rx2+0x3050`) | (−254.05, 40.54, 32.39) | −85.5° | project choice | (0.67, 45.84, 0.92) |
| DownTownSkatePark | `world dist_downtownpark` | `Z_SP_DT_Start` (`PARK/DIST_DownTownSkatePark/DIST_DownTow_[0x4a711835c131688c].rx2+0x3d0`) | (66.05, 0.00, −64.02) | 0° | park selector | (1.43, 1.00, 0.73) |
| Industrial | `fe_locations dist_industrial_reclaimed` | `Z_Ind_ReclaimedHastings` (`BAM/DIST_Industrial/DIST_Industr_[0x468f25c970048ddc].rx2+0x1750`) | (−89.16, 1.01, −138.35) | −45.4° | project choice | (0.47, 4.25, −1.97) |
| IndustrialSkatePark | `world dist_industrialpark` | `Z_SP_Ind_SP_Start` (`PARK/DIST_IndustrialSkatePark/DIST_Industr_[0xa67b50e3de238af8].rx2+0x2d0`) | (−64.01, 0.75, 0.00) | 0° | park selector | (−1.63, 40.32, 1.96) |
| MaloofMoneyCup | `fe_locations dist_maloof_street` | `A_Maloof_Street` (`PARK/DIST_MaloofMoneyCup/DIST_MaloofM_[0xb4028543cd9b301f].rx2+0x250`) | (0.56, −0.06, −43.42) | 180° | project choice | (3.07, 3.12, −2.75) |
| MegaPark | `world dist_megapark` | `Z_MegaPark_SkatePark` (`PARK/DIST_MegaPark/DIST_MegaPar_[0xc0eb00ffefbfeae4].rx2+0x250`) | (230.63, 0.48, −73.08) | −90° | park selector | (21.74, 21.61, −19.68) |
| SkateSchool | `fe_locations dist_skateschool, schoolstart` | `tele_world_to_school_dest_locator_01::tele_world_to_school_dest_spawnpoint_01`, slot 0 of the record at `+0xcd0` (`PARK/DIST_SkateSchool/DIST_SkateSc_[0x54d11766c4a11cd8].rx2+0x1a70`) | (−31.27, 0.11, −28.50) | 0° | `schoolstart` row | (0.82, 7.63, −0.65) |
| StartPark | `world dist_startpark` | `Z_SP_SkateParkStart2` (`PARK/DIST_StartPark/DIST_StartPa_[0x18f5bf0dd72c75c8].rx2+0x250`) | (−64.01, 0.00, 0.00) | 0° | park selector | (1.52, 33.22, −9.78) |
| University | `fe_locations gamestart` | `ftl_univ_02_challengelocator_01::ftl_univ_02_spawnlocator_01`, slot 0 of the record at `+0x2850` (`BAM/DIST_University/DIST_Univers_[0x0f7a42a9282864ac].rx2+0xe4b0`) | (299.19, 74.12, −434.68) | −65.5° | `gamestart` row | (330.00, 133.01, −710.00) |

Paths are relative to `data/content/global_locators/`. Every previous spawn had
heading 0. With the previous spawns MegaPark and StartPark fell without
landing (`PhysicsGround->PhysicsAir` at tick 9; about five seconds later the
game reset the skater to the spawn, and the fall repeated). IndustrialSkatePark
started on a roof, SkateSchool on top of the hub wall and University on a
plateau far from the campus.

The game places the wheel bottoms at the start height. Most starts sit on
the floor (within 0.12 m). Four sit higher: MegaPark 0.48 m, IndustrialSkatePark
0.75 m, DownTown 0.32 m and Industrial 0.27 m, so the board drops briefly
before it lands. MaloofMoneyCup's sits 1.2 cm below the floor. Whether the
original's placement (`+0x14` slot 8) snaps to the ground is not traced; the
port keeps the authored height.

## Surfaceless collision meshes (provisional)

`skate_data::retail_collision::visit_clusters` skips a clustered mesh in which
no unit has a surface ID (unit flag `0x80`). The rule is empirical: the
retail code that keeps these meshes out of world collision has not been traced.
It is per mesh because meshes mixing surfaced and surfaceless units are
ordinary geometry (DownTown has 138).

Survey of the `Sim` streams of all ten districts, using mesh-header bounds:

| District | Surfaceless meshes | Matching EB0019 trigger volumes (by bounds) |
| --- | --- | --- |
| SkateSchool | 7 | `coach_frank_sksc` / `ws_sksc_coachfrank_instance_01` (3 identical meshes), `tut_sksc_inthehub_vol_01`, `tut_sksc_inthehub_vol_02`, `tut_sksc_reset_vol01`, `tut_sksc_wrongway_vol_01` |
| MaloofMoneyCup | 2 | `tele_mega_ramp_up_volume_01`, `tele_mmcp_to_dwtn_volume_01` |
| DownTown | 2 | `dwtn_sessionspot_01_kubetower_volume_01`, `dwtn_sessionspot_02_spillway_volume_02` |
| MegaPark | 1 | `tele_stadium_to_world_volume_a` |
| Other six | 0 | none |

Each is a 12-triangle box, and each box's bounds equal an EB0019 item's
bounds (item `+64`/`+80`). SkateSchool's start (−31.27, 0.11, −28.50) lies
inside `tut_sksc_inthehub_vol_02` (X −79.11…16.52, Y −1.53…6.65,
Z −43.29…52.33), and DownTown's `Z_DT_CubeTower` destination lies inside the
Kube Tower box, so these boxes cannot be solid. Loaded collision triangles
(`SKATE_RWCM_READY`) change only in these four maps: SkateSchool
91,790 → 91,706, DownTown 1,545,276 → 1,545,252, MaloofMoneyCup
36,163 → 36,139 and MegaPark 12,170 → 12,158.

**Open question: can the links identify trigger meshes instead?** In the
data, every clustered mesh has its own EB000A link record; link `+12` names an
RW volume of type 6 whose `+68` is the mesh's section. The link count equals
the mesh count in every district, so a link alone does not mark a trigger.
EB0019 items (`+220`) reach 11 of the 12 boxes. The twelfth, the third Coach
Frank box (SkateSchool arena `0x3F7E34C52515969C`, section 12), is reached
through link section 13, which the arena's EB0029 script record for
`coach_frank_sksc` references at `+0xB8`. A rule that excludes only meshes
linked from EB0019 items would leave that box solid. The retail registration
of world collision has to be traced on the base disc before the empirical rule
can be replaced.

## Host adaptations

- One fixed start per map, player 0's placement. Retail chooses a start per
  session; the travel menu (`teleports.json`) remains the way to reach other
  locations.
- `--spawn-offset`, a network-multiplayer option, moves this instance's
  player along the deck's right axis (basis column 0) instead of world X, so
  players still stand side by side at any heading. At heading 0 the result is
  unchanged. The original
  places online player `i` at spawn slot `i`, or 5 m apart along world −Z
  (`828E9688`); the `.skate` header carries no slots, so the offset stays a
  host rule.

## Validation

- Python: `tools/asset_pipeline/test_map_starts.py` (park root row and flag,
  chosen and only rows, per-district records, first spawn slot, ambiguity) and
  `test_map_writer.py` (header spawn and heading, render-only zeros, missing
  spawn rejected). Rust: `skips_meshes_without_any_surface_ids` in
  `crates/skate-data/tests/retail_collision.rs`.
- All ten maps were converted with `tools/asset_pipeline/map_job.py`.
  Compared with the installed conversions, each differs only in the four
  header floats (spawn and heading) and in one key of the embedded manifest
  (the conversion directory); geometry, textures, rails and the collision
  archive are byte-identical.
- `skate3rust --verify` on each converted map, with no controller input: all
  ten end in `PhysicsGround` with wheel contacts at the start, facing the
  authored heading. `--spawn-offset 2` on MegaPark (heading −90°) lands 2.0 m
  along +Z, and `--teleport dist_university_library` still reaches its
  locator.
- The ignored stock-asset test
  `private_extracted_map_supports_production_gameplay_startup` passes on nine
  converted maps. IndustrialSkatePark fails its 24-tick wheel-support check
  because the board is still falling from 0.75 m; the test failed on the
  previous spawns of all five maps tried (1 m drops, or no support at all).

## Remaining evidence

- Callers of the setter `8283F758`, to confirm how a non-park arrival picks
  its `fe_locations` row, and the meaning of `825F1600`'s argument.
- The owner's `+0x14` object and its slot 8 in `82872F00`: whether placement
  snaps to the ground.
- How `828E8B68` chooses the world's locator directory.
- The retail rule that keeps trigger meshes out of world collision.
- Which object `8286FFF0` returns in `82872D80`; its transform takes
  precedence over the start locator when it exists and passes the check.
- Whether travel reaches its destination through the same spawn object. If
  it does, travel also lands on slot 0: `teleports.json` uses record
  matrices, and one of its 41 placed destinations (`dist_skateschool`) owns a
  slot.
