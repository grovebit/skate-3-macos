# Board collisions

Board contact producers, grind starts and ordinary deck impacts. Part of the [audio research](README.md).

## Board collision producers (contact tags ported)

Static traces of the same base-disc executable (SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`).
The retained truck/deck tags, deck-strength calculation, four-publication
peak history and ordinary deck selector are implemented. Deck playback uses
the original collision path with a current export; trucks retain host onset.

`82474D30` has 14 call sites. Besides the body loop and `824AAD48`, they are
`824A84C8` (landing), `824A8F78` (grind start), `824AAE98` (deck impact),
`82484098` (via `824833F8`) and `824D16E0` (another object). Contacts update
`824A60B0` runs its steps in this order: `824A7438`, `824A77E0`, `824A91C8`
then `824A6578`, `824A6F70`, `824A93D8`, `824A99C0`, the body loop `824AA020`,
`824AAE98`, `824AB1F0`, `824A6448`, `824AD8E0`, `824AE080`, `824AE670`,
`824AEC40` and, after a counter update at `+1B8`, `824AE950`.

**Deck impact, `824AAE98`.** This mirrors one body-loop region:

1. Skip unless strength (snapshot `+29C`) is above 0 and the timer at
   Contacts `+124` is not positive.
2. The board side is board `0x5F`, or boardtumble `0x71` when snapshot `+2CC`
   or `+2A4` is set. The surface is the material at snapshot `+294`.
3. Classify both sides (`82484EC8`). A side whose material is `0x8F` or above,
   by signed compare, is category 3. Continue only if either side is audible.
4. Set the timer to `aud_collisions/default` `Hash_27D3C5DC3282B59D` (Int32 6),
   converted to float.
5. Enqueue the pair with levels from `82484A98` and flags
   `[0, owner +48 byte, tail]`, at position snapshot `+90`.
6. For hard sides, enqueue a medium layer with levels scaled by `82484D90`,
   flags `[0, 0, tail]`.
7. After that, a positive timer loses `min(snapshot +DC, 1)`, with no zero
   clamp.

There is no settled-wipeout gate. `tail` is the body loop's owner condition.

Its inputs:

| Snapshot | Packet | Source |
| --- | --- | --- |
| `+29C` strength | `+1E4` | Audio `+24`: maximum of Collision `+18` over the last four publications (`8274F578..8274F5EC`; the publisher keeps them at its `+284..+290` with index `+294`) |
| `+294` surface | `+1DC` | Collision `+0C` tag converted by `8277EF94..8277EFBC`: tag 0 or above `0x90` gives `0x8F`, otherwise tag − 1 |
| `+2CC` | `+98` bit 30 | Set when `82D02C48` (state category) of State `+10` is 500, the off-board states (`8277E788..8277E7A8`) |

Collision `+18` is written by `82BDAF44..82BDAFB8` in the board publication
routine `82BDA558`. When the deck contact byte (board body `+352`) is clear it
is 0. Otherwise it is |`vmsum3fp`(deck acceleration body `+210`, deck contact
normal body `+C0`)| × 0.00125 (constant `822F392C`), clamped to [0, 1] with
two `fsel`s; NaN gives 1.

Collision `+0C` is read from field `+2E8` of the deck part object, which is
the board body + `0x40`, so it is board body `+328`. Report loop
`82BDFC8C..82BDFCB8` stores each truck or deck report's tag & `0x7F` at body
`+310 + 4 × part`, last report wins. Front truck, back truck and deck land at
`+320`, `+324` and `+328`, published as Collision `+04`, `+08` and `+0C`. No
per-frame reset was found, so the deck tag is retained between contacts.

These offsets match the port's TU3-labelled board fields:
- Contact bytes `+34C..+352` are counted into `+364` (`82BDFF28..82BDFF6C`),
  as in BoardBody 844..850 and 868.
- Accelerations are at `+1B0 + 16 × part` (BoardBody 432..528).
- Per-part contact normals are at `+60 + 16 × part`, so `+C0` is the deck's
  own contact normal, not the filtered ground normal.
- Part-object `+30F/+310/+311` publish to Collision `+CE3/+D91/+D92`, the
  wheel-3 and truck contact flags the port already publishes.

**Grind start, `824A8F78`.** It is called by `824A6F70` when snapshot `+155`
(packet `+94` bit 25) rises and timer Contacts `+7C` is not positive:

- Material is board `0x5F` when snapshot `+C0` (packet `+60`) is 1, 2 or 5,
  and truck `0x60` otherwise.
- The surface is snapshot `+2B4`, which is Audio `+28` converted like the deck
  tag; `0x8F` becomes `0x0A`.
- Strength is snapshot `+E4` (packet `+70`). Above `aud_rails/default`
  `Hash_086B66C3D4FFEE8F` (0.25), both sides are category 1 over
  [0.25, `Hash_B2ACAFDBCD963C93` (0.5)]. Otherwise both are category 0 over
  [0, 0.25].
- One record is enqueued with flags `[0, 0, tail]`, and `+7C` is set to 0.5
  (`82099700`).

Further base-disc tracing (the same `default.xex` SHA-256 recorded above)
establishes the publication and latch behavior:

- `8274FE90..8274FF58`, called by `8274F410` at `8274F570`, reads the
  publication table's Grinds member `+10`. While Grinds `+13C` is nonzero,
  it retains family `+88` at owner `+2A0` and surface `+D8` at owner `+2A4`.
  Otherwise both remain unchanged. It publishes these to Audio `+1C/+28`.
- Audio `+D0` is true when State `+0C` is 400, or when State `+10` is 701
  and Grinds `+143` is nonzero. `8277E970..8277E994` copies that flag to
  packet `+94` bit 25, Audio `+1C` to packet `+60`, and Audio `+20` to
  packet `+70`. `8249EF30..8249EF48` reads these into the snapshot.
- `8274FF38..8274FF54` publishes Grinds `+80` to Audio `+20` only when
  positive, retaining that value at owner `+298`. Otherwise it republishes
  the retained strength; constructor `8274F324` initializes it to zero.
  The existing host field is `GrindOutputFields::impact_speed_128`, but its
  physical producer uses title-update research. Base-disc common fill
  `82D15F10` reads its physical state's manager pointer `+10`, then copies
  manager `+5D4` unchanged to Grinds `+80` at `82D15FBC..82D15FC0`. It also
  copies manager `+5BC` to Grinds `+D8`. This matches the host fill's field
  routing. The cited title-update fill address `82D40EA0` points to
  different code in the base-disc executable; no constant address delta
  is assumed for other routines.
- `8277EFC0..8277EFE8` converts Audio `+28` to packet `+200`: tags 1..0x90
  become tag minus one; all others become `0x8F`.
- `824A70C0..824A70EC` calls the onset selector on a rising edge.
  `824A70F0` always saves the current flag to Contacts `+7A`, including when
  cooldown prevents playback. A blocked edge is therefore consumed.
- Cooldown decrements in `824AC8E8..824AC900`, inside rolling/grind callback
  `824AC050`, called by `824ABFC8` at `824ABFFC`. A positive timer loses
  manager `+3C`, without a zero clamp. This is separate from Contacts input
  callback `824A60B0`; their function pointers occur at `822F71CC/+04`
  and `823328B0/823329B0`. The group clock and dispatch are recovered below;
  the deck publication-ratio clock is a different quantity.

`audio::board_contact::GrindPublication` now ports this retained publication
stage separately from event selection. Reset `8274F2B0` sets strength to zero
at `8274F324`, family to `0xFFFFFFFF` at `8274F334`, and surface to zero at
`8274F33C`. This is the same virtual `+08` reached by the ordinary teleport
dispatcher documented below for deck history. Local
`verify_grind_publication.py` compares production Rust against original
`8274FE90..8274FF58` and those reset stores: all 1,216 publications/reset
sequences match, including retained contacts, nonpositive/unordered strength
and both active-state branches. Four board-contact tests pass.

### Grind-start physical producer and group clock

The same base-disc executable supplies the physical producer chain:
`82D5FC30` runs geometry, balance, engagement, controls and jumper publication
in order. Engagement call `82D5FCCC` reaches `82D5BF10`, which clears the
investigation-relative `+184` strength even when the candidate is invalid.
For an admitted ground/air entry, `82D5C028` computes the entry velocity and
returns its delta from the incoming processed board/air velocity at
`82D5C3B8..82D5C3C0`. `82D5C3E0` measures that delta's three-component
magnitude using the native reciprocal-square-root refinement and stores it
at investigation `+184` (`82D5C468`). Thus strength has the incoming
velocity's units, without a gain or normalization before audio classification.
It may remain positive after impact admission requests a wipeout.

The final copy `82D5FCF0 ->82D5CFA8` copies that investigation to processed
input `+450`; its `+184` becomes processed `+5D4`. The common physical fill
then copies it to Grinds `+80` as above. This verifies the host routing from
`Engagement::update` through `impact_speed_1492` and `impact_speed_128`.
This is a per-routine base-disc mapping, not a title-update address offset.
Existing host velocity integration and arithmetic remain adapters; this trace
does not establish bit-exact Xenon vector arithmetic or whole-physics parity.

Contacts virtual `+14` binds the group to Contacts `+10/+1C` through
`824DDCA0`. Group input routine `828B7C58` stores incoming elapsed seconds
at group `+3C` (`828B7C7C`), accumulates `+38`, then invokes object virtual
`+24` (Contacts `824A60B0`) followed by `+30`. Player-family output virtual
`+14`, `828B75D0`, calls each group's virtual `+18`; `828B7C08` invokes
object virtual `+28` (Contacts `824ABFC8`). That callback decrements grind
cooldown using the **last input elapsed**, not the output elapsed. Group
`+3C` is initialized to zero at `828B7740`. Normal manager dispatch runs
input families first (`82473248..82473274`), then output families
(`82473298..824732C0`), so a combined update can decrement a new timer.

### Runtime grind-start playback

`audio::grind_start::GrindStarts` ports the rising-edge latch, selector and
separate output decrement. It preserves board/truck family routing, absent
surface fallback to `0x0A`, the inclusive soft upper boundary, paired category
and owner-tail condition, unconditional enqueue and 0.5 cooldown assignment.
Blocked edges are consumed; there is no positive-strength onset gate.
Physical publication now runs after State, Grinds and conditioning outputs,
and teleport resets retained inputs while leaving Contacts' latch/timer alive.

The export includes optional `settings.grind_bands` from the two authored
`aud_rails/default` fields above. Older exports do not enable this selector.
The installed library was refreshed after matching those collection rows'
source hashes; `collisions.pre-grind-bands.json` preserves prior metadata.
Runtime processes grind onset before body/deck input loops, stores input
elapsed for the later output phase, and shares the collision queue/groups.
Selected records log `SKATE_AUDIO_GRIND_RECORD`.

Local `verify_grind_start.py` compares production Rust with the original
rising-edge instructions, `824A8F78` material helpers and `824AC8E8` decrement.
Across owned profiles and three randomized profile sets, all 6,400 updates
and 500 records match, including positions at snapshot `+30`, authored
boundary neighbors, blocked edges, zero strength, fallback tags and owner
flags. The owned-bank test resolves generated grind records for all six
families across the tested strengths and surfaces.

Playback remains subject to host fixed-tick manager timing, provisional owner
flags (off), MixMap gain/pitch, spatial placement and sample-layer timing.
A deliberate grind entry and listening comparison on a real map remain open;
instruction replay and startup smoke verification do not establish those.

Validation: 69 core audio tests, 43 game audio tests, the owned-bank generated
record/clip check and 13 exporter/voice tests pass. The final macOS University
smoke run loaded the collision library and ended with `GAME_VERIFY_OK` after
startup, teleport and grounded simulation; it did not exercise a deliberate
grind entry.

`824A6F70` also calls `824A7B60` when airborne flag `+14C` rises, provided
`+157` is set, the state at `+15C` is not -1, `0x1F`, `0x20`, `0x23` or `0x24`,
and no grind is latched. It calls `824A84C8` when `+14C` falls with `+155`
clear. `824A84C8` contains the wheel-contact selector and one more enqueue
(`824A8A74`), which has not been examined.

`BoardGroundState::audio_tags` now retains the three actual solver-report
tags, masked to seven bits. Contact-free updates preserve them; each part's
last report wins independently of the selected normal. `publish_board_input`
copies them into Collision `+04/+08/+0C`, matching base-disc
`82BDAA8C..82BDAAAC`. The existing report/part mapping supplies front truck,
back truck and deck at indices 4, 5 and 6. No base-disc audio addresses are
treated as TU3 addresses.

The local `verify_board_audio_tags.py` executes the owned
`82BDFC8C..82BDFCB8` instructions for all 65,536 report tags on each of the
three parts, plus the three original output copies. Rust checks the same
mask domain and contact sequences (contact loss, report ordering, normal
selection independence, wheel exclusion and a zero-tag replacement).
Startup/full physical reset zeros these tags: base-disc CollisionInfo
constructor `82BD8840` receives BoardBody `+40`; its seven-iteration loop
stores zero to `+2D0 + 4 × part` at `82BD88B0`. The board teleport
`82BDDA28` calls `82BE5158`, which calls this constructor at `82BE5230`.
This independently confirms the zero initialization/reset used by the host;
it does not equate base-disc and TU3 function addresses.

`BoardGroundState::deck_impact_strength` now publishes Collision `+18` from
the completed solve's deck acceleration and selected deck contact normal.
The existing acceleration producer samples all seven part velocities after
integration and divides their change by the physics timestep; no solver
impulse, closing-speed scalar or overall ground normal is substituted. The
constant at base-disc `822F392C` is binary32 `0x3AA3D70A` (0.00125).
`82BDAF58..82BDAFB8` gates on deck contact, clears the dot product's sign bit,
multiplies by that constant, then applies two `fsel` boundaries. Contact loss
produces positive zero; unordered input produces 1. The zero/one operands
are verified at `82163B30` and `82314D90`.

`audio::board_contact::DeckStrengthHistory` ports `8274F578..8274F5EC`:
replace the indexed slot, advance modulo four, then scan slots 0 through 3
with ordered greater-than comparisons. `8274F330` initializes the index to
zero and `8274F358..8274F3A4` clears all four slots. Current materials are
not aged alongside strengths. `8277F000..8277F008` copies Audio `+24` to
packet `+1E4`; `8249F224..8249F228` copies it to snapshot `+29C`.

The host `PlayerInputRuntime::publish_board` advances this history once after
each completed board output publication; audio-manager phases do not advance
it. The ordinary deck selector now consumes the result. Native reset wiring
is established below; outer scheduling still has host adapter differences.

Local `verify_deck_strength.py` compares production Rust with the original
instruction regions for 2,031 composed strength/history publications. Cases
include random vectors, contact-free publications, positive/negative strength,
signed zero, saturation neighbors, infinities, NaN and peak expiry. Its VMX
dot operation uses the existing host binary32 arithmetic convention, so these
comparisons verify instruction data flow and scalar/history boundaries, not
bit-exact Xenon dot rounding. Host tests also check finite-difference inputs,
deck-normal selection, tangential motion and physics-output publication.
No end-to-end board-impact parity is claimed.

### Publication-owner dispatch and teleport reset

Base-disc `82DC7480` constructs the eleven-component publication table.
Its member `+0C` receives a `0x320`-byte owner with vtable `8230B0E4`:
virtual `+04` is update `8274F410`, virtual `+08` is reset `8274F2B0`.
Construction also calls reset directly at `82DC75BC`.

`82DC7A60` calls every component's virtual `+04` exactly once, in table
order; the deck owner call is `82DC7AC0..82DC7AD4`. PhysicalPlayer's
`82D8D838` forwards its table at `+764` to that dispatcher and occupies
vtable `82322B88` slot `+44`. The owner update reaches the history kernel
unconditionally after its two earlier helpers and body publication. This
establishes one history advancement per completed dispatch, not a fixed
wall-clock duration or a complete audit of outer world scheduling.

Ordinary teleport `82D8DC98` resets that same publication table at
`82D8E0B8..82D8E0BC` through `82DC7B98`. It calls each component's virtual
`+08`; the deck owner's call is `82DC7BE8..82DC7BF8`. Thus teleport clears
the history slots and write index. The host now calls
`DeckStrengthHistory::reset` at its corresponding completed-teleport boundary,
before the next physics publication. Contacts' deck audio timer is a
different owner and survives this physical reset.

Local `verify_deck_publication_reset.py` executes both original dispatchers
with component boundaries hooked, then the original deck index/history reset
stores. Five dispatches each called the deck owner once; the reset dispatcher
cleared all four seeded slots and the index. This is instruction replay,
not original-hardware or real-map verification.

### Runtime ordinary deck-impact playback

`audio::deck_impact::DeckImpacts` ports `824AAE98..824AB1EC`, reusing the
verified material classifier, levels and hard-layer scale. Its ordered
records use board `0x5F` or boardtumble `0x71`, the current deck surface,
owner flags and optional medium overlays. Cooldown remains Contacts `+124`:
it is assigned and decremented in the same input-family update, with the
original `min(publication_ratio, 1)` boundary and no zero clamp.

The exporter now includes `settings.board_cooldown`, read as Int32 from
`aud_collisions/default` `Hash_27D3C5DC3282B59D` (6). Existing libraries
without that field retain the provisional deck onset path. The installed
local manifest was refreshed from the verified owned collections after
matching its five database source hashes; its previous metadata is retained
as `collisions.pre-deck-cooldown.json`.

On Contacts input phases, runtime updates the body loop then the deck loop,
matching `824A60B0` ordering, and shares the collision groups/consumer. Only
deck reports are removed from the provisional observer when this bank field
is present; truck, foot and independent wheel behavior stays available.
Playback logs deck records as `SKATE_AUDIO_DECK_RECORD`.

Local `verify_deck_impact.py` executes the original deck routine and material
helpers with the body oracle's existing database/enqueue boundaries hooked.
Production Rust matched 10,180 updates and 3,070 records across owned profiles
and four randomized profile sets, including authored boundary neighbors,
cooldown expiry, silence, absent surfaces, tumble flags, owner flags and hard
overlays. Record positions matched snapshot `+90`. The owned-bank test also
checks ordinary/tumble generated records against actual exported event clips.

Remaining adapter limits: Contacts input updates use the host fixed tick and
existing manager phase clock; publication ratio is still 1 and owner flags
are still off. Live MixMap volume/pitch controls, spatial position, original
layer timing/randomizers and a real-map listening check remain unverified.
The record selector is verified separately from those playback dependencies.

Validation: 66 core audio tests, 43 game audio tests, the owned collision-bank
check (including generated deck sides), and 12 exporter/voice tests pass.
The macOS University smoke run loaded the 280-event collision bank and ended
with `GAME_VERIFY_OK`; it checked startup, teleport and ordinary grounded
simulation, not a deliberate deck hit or listening comparison. Its sandboxed
attempt could not access Metal; rerunning with GPU access passed.

Remaining port, in order:
1. Check deck hits on a real map by ear and against `SKATE_AUDIO_DECK_RECORD`.
2. Recover the remaining publication ratio/owner and outer timing producers.
3. Check grind entry on a real map and trace remaining landing enqueue.
