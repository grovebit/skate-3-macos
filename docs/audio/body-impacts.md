# Body-impact audio

Ragdoll and body impacts: material evidence, event selection, cooldowns, original record generation and runtime playback. Part of the [audio research](README.md).

## Body-impact audio investigation

This section records the investigation in order. The original body loop is now
ported and drives head, torso, arm and leg playback when `collisions.json` is
present; see [Runtime body-impact playback](#runtime-body-impact-playback) for
its current inputs and gaps. The statements below about host playback describe
the path that remains for board and foot contacts, and for body contacts
without that file.

Before that port, the host tested solver impulse / time step against
provisional thresholds of 20 for body contacts and 32 for feet, with a shared
square-root gain formula. These are not recovered original rules. Missing ragdoll audio must be diagnosed
through this path separately from wheel landings; the wheel port does not
establish body-audio parity.

In the same base-disc executable, the loop around `824AA1A8..824AA9C0`
reads six body-group strengths at audio snapshot `+1F0`, and contacted
surfaces at `+230`. `824AAA38` selects body-related audio materials.
`82484EC8` classifies the strength independently for each material through
its `aud_intensitymapping` reference (`Hash_E228508FE0F53970`). The classifier
returns three audible intensity categories or a silent category. The loop
also maintains per-group cooldowns and can enqueue additional layers through
`82474D30`.

The owned database maps head to `face_head`, torso to `torso`, arm to `arms`,
leg to `limbs`, and foot to `dmo_sk8board` intensity collections. These tables
are available, and the original strength producer is now implemented below.
The playback selector still uses host impulses; substituting the tables directly
for its impulse thresholds would not reproduce the original behavior. This investigation has not yet reproduced or fixed the
reported silent bail.

The exporter now preserves 145 ordinary `aud_material` references to 45
`aud_intensitymapping` collections under `material_intensity` in newly prepared
manifests. The runtime consumes the primary body profiles for optional diagnostics;
audible playback still uses provisional rules. Optional special-mode references (`Hash_F64F891C312EFD5E`) remain unported.

Schema offsets and classifier branches establish these meanings:

| Boundary | Field hash | Schema offset | Original comparison |
| --- | --- | --- | --- |
| Minimum | `D660AC459139BDF4` | `+0C` | Strength below this is silent |
| Medium | `7D8DEDD338D45482` | `+08` | Strictly greater selects category 1, unless above hard |
| Hard | `C8DED1BC20B9D6A5` | `+00` | Strictly greater selects category 2 |
| Upper scale | `B8870D2001033E0F` | `+04` | Upper interpolation bound for category 2 |

At the minimum itself, category 0 is audible. Equality at medium remains
category 0; equality at hard remains category 1 for the usual ordered bounds.
These differ from the wheel classifier's inclusive medium/hard boundaries.
Export retains the authored values without sorting or normalizing them.

The original physical part-to-region table at base-disc `820CFAD0` matches the
existing TU3 `820CFCB0` physics table. Audio now shares that table through
`SkeletonCollisionFeedback::contact_region`, correcting parts 6 and 10 from
arm to torso. Left/right limbs still share provisional audio channels.

The strength producer at `82BAE1D8..82BAE308` uses the selected part for
each of eight collision regions. For a present contact, it calculates:

```text
normal_change = abs(dot3(SkeletonState.velocity_changes[part], region.normal))
strength = clamp(max(0.001, normal_change * raw_part_weight * authored_scale), 0, 1)
```

Missing contacts produce zero, without the minimum floor. `SkeletonState+FD0`
contains pose-derived velocity changes, and `+11D0` contains raw part weights;
these are separate from the normalized center-of-mass fractions. The scale is
`physics_collision/default` field `Hash_1430BD50F0A33475`, layout offset `+A4`,
with authored value 10. Neither solver impulse nor the feedback's weighted-force
array is the input to this calculation.

`8274FF60..8275004C` retains four publications and takes the maximum of each
of the eight strengths. Material tags remain from the current publication,
even when its strength comes from an older peak. The filtered block flows from
publication table member `+28`, offset `+5C`, through packet `+140`
(`8277EDB4..8277EDD4`) into audio snapshot `+1F0` (`8249F178..8249F198`).
The snapshot publisher then multiplies all eight strengths by a speed graph,
up to ×5 and without a clamp, before Contacts reads them
([PlayerPhysics speed graph](#playerphysics-speed-graph-on-body-strengths)).
The body selector consumes the first six regions; the two foot regions have
separate consumers. The history advances per physics publication, not by an
assumed wall-clock duration. Original owner reset/teleport semantics remain
unverified.

`BodyContactAudio` now implements this calculation and peak history, called
after the completed physical solve using the loaded authored scale. Tests cover
normal versus tangential change, raw weighting, the minimum floor, saturation,
peak expiry, and current-frame materials. The macOS verification run loaded the
setting and completed with `GAME_VERIFY_OK`; it did not exercise a ragdoll fall.
This establishes the input implementation, not audible body-impact parity.
Playback still needs the original material-pair selector, gain interpolation,
cooldown clock, and additional layers.

For diagnostics, launch with `SKATE_AUDIO_BODY_TRACE=1`. Each fresh contact
publication logs `SKATE_AUDIO_BODY_INPUT` with current and filtered strengths,
material tags, and the provisional host impulses. Native arrays contain eight
regions (head, torso, left/right arms, left/right legs, left/right feet); the
host comparison has six channels (board, head, torso, arm, leg, foot), so their
indices are not interchangeable. These diagnostics are disabled by default.

### Body material volume evidence

In the same base-disc executable, `82484C38..82484D88` selects the ordinary
`aud_material` volume RefSpec at layout `+00`, field `Hash_82B1451A90152514`.
Its target class is `aud_volumemapping` (`13E20D398E385A56`). The optional
special-mode override uses `Hash_E007F3FC90E887D0`; body-loop calls inspected
at `824AA484..824AA4A8` pass the ordinary mode.

`82484A98..82484C34` receives the category's lower bound in `f1`, upper bound
in `f2`, and contact strength in `f3`. Negative material IDs and silent category
3 return zero. Otherwise, it selects integer level endpoints from these offsets:

| Category | Counterpart class 0 | Counterpart class 1 | Counterpart class 2 or greater |
| --- | --- | --- | --- |
| 0 (soft) | `+08/+0C` | `+18/+1C` | `+34/+38` |
| 1 (medium) | `+00/+04` | `+10/+14` | `+2C/+30` |
| 2 (hard) | `+20/+24` | `+20/+24` | `+20/+24` |

Counterpart class comes from `82485750` only when the primary material is below
`61` and the other material is not `8F`; otherwise it remains zero. These classes
must not be interpreted as physical surface hardness. Body material IDs start
at `61`, so their side of the pair uses class zero.

Strength is capped at the upper intensity bound. When upper is greater than
lower, the routine computes `(level_max - level_min) / (upper - lower)` in
single precision, performs a fused multiply-add with `(strength - lower)` and
`level_min`, then truncates toward zero to an integer. Degenerate or reversed
intensity bounds return `level_max`. There is no square-root curve in this
routine. This result is an intermediate integer level, not yet a verified
conversion to the host audio backend's gain.

The exporter now preserves all 14 integer endpoints per volume profile under
`material_volume`, sharing typed scalar and RefSpec validation with intensity
export. The owned database yields 76 profiles and 145 material references;
`default` and `hom` have null target hashes, which remain JSON null rather than
being assigned an invented fallback. Original resolution of those null profiles
is not yet traced. For example, head selects `body_collision_head`, whose ordinary
soft endpoints are 10000/17000, medium 10000/23000, and hard 20000/26000.

Independent fixtures check field ordering, integer preservation, descending
levels, null references and invalid data. Adding both intensity and volume tables
to the installed manifest produces 372377 bytes, below the runtime's 512 KiB
limit (372504 bytes with updated limitation text in the installed diagnostic manifest).
The runtime now loads the six primary-body profiles and evaluates their categories
and integer levels when `SKATE_AUDIO_BODY_TRACE=1`. `body_layers` contains a `primary` classification and a `medium_overlay` per
region. Primary `Some((category, level))` denotes an audible material classification;
`None` denotes silence. An outer `None` means profiles were unavailable. These
are raw primary-material decisions, before the body's positive-strength gate,
state gates, cooldowns, surface-side classification, and additional layers.
They are not scheduled voices. Missing/null primary references or invalid scalar
values disable this diagnostic bank, preserving the existing playback path.

The evaluator preserves strict medium/hard comparisons, hard-before-medium
branch order, upper capping, fused multiply-add and truncation, and the original
handling of reversed or degenerate interpolation bounds. Tests cover those cases,
descending volume endpoints, material reference resolution, and invalid inputs.
The installed profiles were regenerated from database files whose SHA-256 hashes
match the existing audio manifest's provenance. The macOS smoke test loaded this
bank and emitted diagnostic levels before `GAME_VERIFY_OK`; it exercised startup
and grounded silence, not a ragdoll fall. All 35 Rust audio tests and 20 focused
exporter tests passed.

The next body-loop branch, `824AA590..824AA678`, adds a second category-1
(medium) layer for each side of a pair classified as category 2 (hard). Other
sides use silent category 3 in that additional record. It scales the previously
computed hard integer level; it does not interpolate medium endpoints again.
`82484D90..82484DD8` reads the scale at volume-profile `+28`, field
`Hash_1A5F7E8CCABBB0A2`. Integer-to-float conversion, single-precision multiply,
and truncation produce the second level. Zero levels are retained at this stage;
whether downstream playback discards them is a separate question.

Export now includes `hard_layer_scale`, and the diagnostic primary-body evaluator
reports this extra layer as `medium_overlay`. The owned profiles use scales 0 for
head/legs and 1 for torso/arms; all 76 profiles have scales between 0 and 1.
Older exports without this field still evaluate their primary level and report
no overlay, without inventing a scale. Tests cover that compatibility, the hard
threshold, fractional truncation, zero scaling, and scales above one (no inferred
upper clamp). The surface side, remaining body layers, scheduling, and voice
conversion are still outside this diagnostic evaluator.

Cooldown decrement is now located at `824AA9C0..824AA9EC`: after processing each
region, a positive timer loses `min(snapshot[+DC], 1.0)`, with no zero clamp. Thus
a newly assigned timer is decremented in the same update. Snapshot `+DC` defaults
to 1 (`8249EA58..8249EA60`) and is filled from the global publication structure
at `8249EE30..8249EE34`. The producer and update cadence of that value still need
tracing before using a host duration; the authored value 15 must not be treated
as seconds or milliseconds. Playback cooldowns, surface-side classification,
additional layers and voice-level conversion remain unported.

### Queued collision playback and event selection

The base-disc consumer at `824BFE08..824BFF84` processes both sides of the queued
0x30-byte collision record independently. It skips material `8F` or category 3,
then calls `82484410` to resolve the bank, event and material controls before
creating a voice. `82484410` delegates event selection to `82484638..824849C4`.
The table at `82FD1930` identifies primary body materials `61..64` as bank 0
(`Skate_Collisions`). The bank-0 event selector uses these `aud_material` fields:

| Category | Counterpart class 0 | Counterpart class 1 | Counterpart class 2 |
| --- | --- | --- | --- |
| Soft (0) | `BFABF634D2B1E45A` (`+40`) | `9ABFC64574AB2F9F` (`+6C`) | `BCD5E888294F7B15` (`+78`) |
| Medium (1) | `EF9BD81F9CFF725F` (`+3C`) | `A3ADCA7B19287B5D` (`+68`) | `C676C87F862C0490` (`+74`) |
| Hard (2) | `9203DF6FD029B377` (`+70`) | Same | Same |

Unlike the volume-profile lookup for body materials, the event selector does
consult the counterpart's class. Bank event zero is valid and must not be
interpreted as a missing event. Other bank branches and special overrides remain
separate porting work.

The exporter now writes a `body_events` matrix for head, torso, arm and leg and
prepares every selected event through the existing layer/variant renderer. The
owned data references 12 distinct events and produces 48 rendered body clips.
For example, torso uses events 949/950/951 for soft/medium/hard. Arm uses 956 for
soft, 957 for medium against counterpart classes 0/1, and 1030 for medium against
class 2 or hard against any class. These matrices are prepared assets; playback
still selects the older representative body routes until the remaining consumer
inputs and timing are connected.

A full preparation to `.local/body-events-validation` succeeded with 365 decoded
source clips and 337 unique rendered clips. All 48 body clips are present and
have corresponding gain trims. The generated manifest is 435652 bytes, below
the runtime's 512 KiB limit; all rendered WAVs total 11043770 bytes. The four new
fixture tests cover category/counterpart routing, valid event zero, simultaneous
layers, and invalid or missing material data. The focused exporter suite passes
25 tests. No additional simplification was warranted in review.

The voice update at `824C01B8..824C0478` reads queued integer levels at record
`+20/+24`. The normalization constant at `822F3538` is float bits `38000100`
(approximately `1/32767`). `824C0334..824C03DC` preserves two truncation steps:

```text
stage1 = trunc_f32((queued_level * normalization) * controller_level)
stage2 = trunc_f32(stage1 * (material_base_level * normalization))
voice_gain = stage2 * normalization
```

`material_base_level` comes from `aud_material+34`, field
`Hash_875BA75341DC8391`, clamped to 0..32767 by `824844D4..824844F4`.
`controller_level` comes from the controller selected by `824BFF88` using the
material's classification. Controller producers/lifetimes still need tracing;
substituting 32767 or the host master volume here would not establish parity.
The routine also sets pitch and other voice parameters before calling
`8294E070`. Thus the integer material level is not, by itself, the final gain.

### Collision controller inputs

The collision voice object's vtable at `822F73A0` dispatches slot `+3C` to
`8249D138` (integer controller lookup). `824BFF88..824C006C` selects the lookup
index from the material class at `aud_material+48`, field
`Hash_D5EF686287A57AFE` (`Sk8::Audio::eMaterialNicotineType`). Classes 0..9 select
controller slots `[13,14,15,16,17,18,12,19,20,21]`; other values take slot 20.
The native comparison is unsigned, so negative enum values take that default.

`8249D138..8249D170` follows voice `+0C`, then owner `+0C`, to a packed control
word array. For slot `i`, it reads word `i >> 1`, shifts by `16 * (i & 1)`, and
masks to 15 bits. Either missing pointer returns zero. The collision constructor
`824BFA78` initially clears its owner pointer; the owner/control producer and
lifetime must still be established before supplying these values in the port.
A missing controller is not evidence for assuming full-scale volume.

`body_controls` in newly prepared manifests now preserves the original base
level, its native 0..32767 clamp, the integer pitch, material class, and selected
volume-controller slot. The owned primary-body values are:

| Material | Base level | Pitch | Material class | Volume-controller slot |
| --- | --- | --- | --- | --- |
| Head | 18000 | 4096 | 5 | 18 |
| Torso | 32767 | 3796 | 5 | 18 |
| Arm | 32767 | 4096 | 5 | 18 |
| Leg | 32767 | 4096 | 5 | 18 |

These distinct values were absent from the host's generic body gain/pitch path.
They are exported inputs, not yet applied playback gains. Pitch remains an
integer because the voice consumer combines it with another controller value
using the separate `1/4096` scale. Special-mode pitch overrides remain unported.
The four new exporter tests cover signed source preservation, clamp boundaries,
all controller-slot mappings, and malformed/incomplete input. All 29 focused
exporter tests pass. Adding these controls to the fully prepared manifest yields
436581 bytes, within the runtime limit. Review found no worthwhile simplifications.

The registration at `82FD13A0` identifies this object as `SFXObj_Collision`
(name at `822451C8`, factory `824BF9E8`, constructor `824BFA78`). This narrows
the remaining owner-binding investigation; the factory/constructor alone does
not establish how controller slot 18 is populated or updated.

`tools/audio/check_collision_voice.py` is an offline reference checker for explicit
queued levels and captured numeric controller words. It implements the verified
15-bit lookup and staged gain arithmetic, with single-precision rounding at each
conversion/multiply and integer truncation toward zero. It does not read live
controller state, choose defaults for an unresolved binding, or drive playback.
For example, this is an arithmetic fixture, not a captured original-game trace:

```sh
python3 -m tools.audio.check_collision_voice --level 10002 --base-level 18000 \
  --slot 0 --controller-words 12345
```

It produces controller-stage level 3768, material-stage level 2069, and gain
0.0631427988409996. Combining the factors before truncating would instead yield
2070, so the two truncation stages are observable. Words are unsigned numeric
32-bit values after big-endian decoding; hexadecimal input is accepted. Omission
of `--controller-words` explicitly models a null controller (zero output), while
a provided but too-short capture is rejected. Signed-conversion overflow is
outside the verified domain and is rejected rather than assigned guessed PPC
behavior. Five checker tests cover packing/flag masking, absent versus truncated
captures, arithmetic staging, clamp boundaries and signed truncation. The combined
checker/exporter suite passes 34 tests. This is reference validation, not gameplay
verification; the owner producer and remaining playback integration are still open.

### Controller owner binding

The callback `82473690..824737BC`, referenced by the manager interface table at
`822F69DC`, resolves the owner before the voice reads packed controls. It reads
the incoming controller's ID at `+04`, then derives:

| Lookup input | ID bits / expression |
| --- | --- |
| Family | `(id >> 16) & 0xFF` |
| Manager pointer offset | `4 * (0xA0 + family)` |
| Group ID | `(id >> 11) & 0x1F` |
| Object ID | `(id >> 4) & 0x7F` |
| Group object-list offset | `+24` if `(id & 0xE0000000) == 0x40000000`, otherwise `+20` |

From the selected family pointer, it traverses the group list at `+10`, following
links at `+04` and matching each group's `+10` ID. It then traverses the selected
object list, comparing `(object[+18] >> 4) & 0x7F`, and stores the incoming
controller pointer at matched object `+0C` (`82473770`). On failure it invokes
the incoming controller's virtual method at `+14` and returns false. For the
MixMap controller table identified below, that method clears output-buffer
word `+3C` when the buffer exists. Family 3 resolves to manager `+28C`, independently
matching the collision queue's family pointer in `82474DB8`.

The reference checker now accepts optional `--controller-id` and reports this
lookup key alongside the staged gain. For the synthetic ID `0x40038D59`, the key
is family 3, manager offset `0x28C`, group 17, object 85, object-list offset `+24`.
Tests check ignored flag bits, every top-three-bit pattern, maximum field values,
and malformed IDs. The combined checker/exporter suite passes 36 tests; review
found no changes worth making. This decodes the verified routing and documents
the binding callback; it does not resolve a live object or populate its buffer.
The remaining dependency is the controller buffer's producer, initialization,
and update/lifetime behavior, followed by playback integration. The next trace
below identifies attachment, initialization and packed writes; expression
evaluation and its live inputs remain incomplete.

### MixMap control program

The original manager loads `data/audio/MixMapSK8.mxb` at
`82472F88..82472FB4`, then constructs its evaluator through `829242C8`.
The owned file is 25,252 bytes, SHA-256
`105f46bbc4ae25cf51bafc8524e00b0aef608a3aa91424e3d39bbc57fff305c2`.
These addresses use the same base-disc executable identified above.
`82924670` initializes the evaluator, builds its groups, and activates updates;
`82924398` dispatches updates to evaluator method `82927E90`.
This establishes a control-program dependency, not a constant controller value.

`tools/audio/inspect_audio_mixmap.py` reads the verified directory and each group's
`+04` declaration section without executing the program:

```bash
python3 -m tools.audio.inspect_audio_mixmap /path/to/data/audio/MixMapSK8.mxb
```

`82925690` reads the big-endian group count and directory offset at file `+04`
and `+08`; directory entries are file-relative group offsets (`FFFFFFFF` means
absent). `8292A6A8` follows the signed, group-relative section offset at group
`+04`, skipping negative offsets. Declarations begin 16 bytes after the section
header. Each contains an ID, descriptor, and `(descriptor >> 16) & 0x1F`
argument words, matching the variable stride at `8292AD44..8292AD5C`.
The reader preserves these words verbatim. Negative counts are rejected as
invalid inspection input; this is not an emulation of the evaluator's handling
of nonpositive counts.

The owned program contains 14 groups and 363 declarations in these sections.
Group 3 contains ID `98030000`, descriptor `0000FE70`, and ID `90030000`,
descriptor `0001FC7C` with argument `400000C2`. Their expression semantics and
relationship to the live collision-volume slot remain unverified; these raw
words must not be used as final gain values. Other sections, expression
execution, and controller-buffer publication are still unported.

Six synthetic reader tests cover variable argument counts, relative offsets,
absent sections, the owned layout's overlapping unused group/directory word,
and malformed ranges. The combined diagnostic/exporter suite passes 42 tests.
The owned-file inventory is verified; body/ragdoll playback remains provisional
and the missing-impact report is still open.

### MixMap output buffers and body-volume route

The follow-up trace identifies the actual controller table at `8231110C`.
Controller `+08` points to full-word inputs: virtual `+08` (`829244F8`) writes
one input word, and virtual `+1C` (`82A72A00`) attaches that buffer.
Controller `+0C` points to packed outputs: virtual `+20` (`825AD300`) attaches
it. Virtual `+14`/`+18` (`82924640`/`82924658`) clear/set output word `+3C`.
The output evaluator tests that word's low bit before evaluating enabled
routes (`82928484..8292849C`, `82928584..829285AC`). Inputs and outputs are
distinct; copying a raw input into the voice's packed-volume slot is incorrect.

`829253F0..829254EC` finds or creates a controller using the canonical ID
`id & 0xE0FFFFF0`, attaches the output pointer, and invokes the manager binding
callback for a new controller. `82927AD0..82927B4C` calls this helper for output
records, clears the first 15 output words, then writes the helper's Boolean
result to word `+3C`. The helper returns true even after calling the manager
callback, without propagating its return value. This ordering matters: the
callback's failure-side flag clear is not proof that initialization leaves
the route disabled. Subsequent teardown/reset behavior remains unverified.

Output construction at `8292AF28..8292B144` uses group `+10` as a signed relative
section offset. The section header supplies record count and buffer count;
records begin at section `+10` and contain descriptor, value word, controller ID,
then `(descriptor >> 16) & 0xFF` argument words. `8292BAA0..8292BCBC` pairs them
in order with write maps starting directly at group-relative `+14`. Each map
has a header followed by `header & 0xFF` words. Execution at `82928590..82928600`
uses only `header & 0x1F` as its write count and `(header >> 24) & 0xF` as format.
Each write contains slot `(word >> 26) & 0x1F`, argument index
`(word >> 21) & 0x1F`, and a signed low-16 field; the high-bit special path and
format-dependent arithmetic must also be preserved when porting execution.
`82928A40..82928A5C` writes one halfword while preserving its packed neighbor.

The inspector now includes `output_groups`: 262 output records and 393 stored
write words in the owned file. Collision group 3 has five records sharing
controller ID `40030000` (before instance bits are inserted by construction).
The first record at file `4534` has descriptor `C0030300`, value `0000D8F0`,
and arguments `90030000`, `30030000`, `B100304C`. Its map at `45AC` has header
`E003000A`. The body-volume slot 18 write is `4812FBB4`: argument index 0,
signed low field -1100. It shares this route with slots 12 through 20 and a
special slot-0 write. Slot 21 uses a separate record and signed field -1800.
These are authored operands, not final linear gains; upstream expressions,
input producers, conversion tables and update cadence are not yet reproduced.

Four additional synthetic tests verify pairing, signed fields, distinct count
masks, absent sections and malformed buffers. All 46 focused tests pass, and
reuse/quality/efficiency review found no changes worth making. This is an
offline diagnostic extension, not a change to runtime impact playback.

### Enabled format-0 volume conversion

The slot-18 map selects format 0. Its final conversion is now implemented in
`tools/owned_game/mixmap_volume.py`, with an explicit-input CLI:

```bash
python3 -m tools.audio.check_mixmap_volume \
  --table /path/to/owned/mixmap-volume-table.bin \
  --record-level 0 --modulation-level 0 --adjustment -1100
```

Both zero levels in this example are supplied reference inputs, not recovered
live defaults. Omitting `--modulation-level` selects the direct path, which
shares the conversion. The checker does not evaluate a modulation node, output
expression, disabled route, other format, or high-bit special write.

`82928658..82928724` adds the evaluated record level, modulation-node level and
signed write adjustment. The direct path at `829288CC..829289C8` omits the node
level. Both use signed 32-bit wrapping addition and clamp to `[-10000, 0]`.
For the clamped level `L`, the lookup is:

```text
shift, remainder = divmod(-L, 602)
converted = 0 if shift > 15 else table[601 - remainder] >> shift
packed_halfword = converted & 0xFFFF
```

This is the original integer lookup and arithmetic shift, not a fitted
exponential curve. The table is 602 big-endian words at virtual address
`82FBB430` in the same base-disc executable (XEX SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`).
In a **memory-mapped** image with base `82000000`, its offset is `FBB430` and
length is 2408 bytes; this is not a PE disk-section offset or raw-XEX offset.
The owned extracted table has SHA-256
`30c4769614bc8837f9016bc6698826a1c347f4f124b0db1e4cef314748609e65`.
It remains local and is not embedded in source or tests.

With that table, the explicit `0 + 0 - 1100` example writes 9224. At `L = 0`,
the table writes 32730, so substituting 32767 at that endpoint would change the
original result. The bit-shift cutoff is at `L <= -9632`; some table results
already truncate to zero before that cutoff. A local exhaustive check of all
10,001 clamped levels matched the checker against the original reciprocal
multiply (`1B37484B`), signed division and branch targets at `8292895C`, plus
the owned lookup data. This validates the recovered conversion, not a capture
of live gameplay or the full evaluator.

Six synthetic tests cover lookup direction, shift transitions, cutoff,
sum-before-clamp ordering, word overflow, signed shifts, packing and malformed
tables. All 52 focused tests pass. Simplify review found no worthwhile changes.
Next required evidence is the producers of the evaluated output-record level
and modulation-node level, including their update/reset behavior, before this
conversion can replace runtime impact gain handling.

### Collision-position modulation inputs

The body route's modulation reference resolves to group 3's first `+08` node.
`8292B680..8292B924` constructs these records. Their header is followed by
`(header >> 24) & 0xF` mode blocks, each six words. The node input reference is
built at `8292B8D4..8292B8E8`; for the owned collision header `81030000`, the
controller resolver's canonical instance-zero ID is `60030000`. This selects
family 3, instance 0, object 0 in the input-controller list (`group +20`).
Both collision modulation nodes share that controller.

The matching registration at `82FD1240` is **SFXCTL_3DColPos**, tag `00030000`,
factory `824A1550`. Its constructor calls the shared position-controller
constructor `8249BEF8`, then installs table `822F6B28`. That table inherits
publication method `8249CAD8` at virtual `+24` and spatial update `8249C988`
at virtual `+34`. This identifies the producer class; the collision-position
pointer assignment and full spatial arithmetic remain to be traced before
claiming a port of its distance inputs.

The inspector now exports `modulation_groups`: 89 nodes with 89 mode blocks in
the owned program. Collision node 0 at file `4494` has mode word `90030000`,
curve word `66660154`, and raw bound pairs `(1,50), (1,40), (1,35), (1,40)`.
Node 1 at `44B0` has mode word `90030001`, curve word `44440154`, and four
`(4,50)` pairs. The constructor decodes each bound word's low 15 bits before
its high 15 bits (`8292B7F0..8292B8D0`). These are raw authored bounds; the
inspector does not assign physical units or apply them to host coordinates.

For node 0, the selectors at mode bits `12..15` and `8..11` are both zero.
`8292A074..8292A0B8` therefore reads float input slot 1 (`buffer +04`) and packed
phase input slot 3 (`buffer +0C`). Selector 1 would instead choose slots 0 and 2;
larger selectors use constants -1.0 and zero respectively. The phase's bits
`14..15` select among four bound pairs and its remainder participates in the
blend. This is state-dependent evaluation, not one fixed attenuation number.

Publication at `8249CAD8` checks position source `+20`. When absent, it writes
input slots 3 and 2 as integer zero, slots 1 and 0 as float -1.0 (constant
`8216C000`), and clears input word 15's low bit while preserving the other bits.
When present, it sets that bit, publishes `+38` to input slot 11, and invokes
the inherited spatial update; an additional path runs if `+24` is non-null.
This input-buffer enable flag is separate from the collision voice's output
buffer flag, even though both occupy byte offset `+3C` in their own buffers.

The evaluator checks that input flag at `8292A024..8292A038`. Its disabled path
(`8292A5C4..8292A5E0`) sets the modulation node's logarithmic level to -10000,
linear level to zero, and clears its phase and auxiliary level. During mode
changes `82929F00..82929FC4` chooses a matching mode block, falling back to mode
zero. The volume-only port described below now implements this selection.
The spatial-update functions contain VMX128 instructions that ordinary
PowerPC disassembly does not decode reliably. The next trace resolves the
distance inputs with a dedicated decoder; complete directional-phase arithmetic
and shared sum inputs `30030000` and `B100304C` remain unported.

Three new synthetic tests cover multiple modes, input selectors, canonical
binding IDs, bound masks, absent records and malformed ranges. All 55 focused
tests pass. Review clarified the constant meaning of null input-slot fields;
no runtime audio behavior changed in this step. Addresses and owned-file
hashes are the same base-disc sources recorded above.

### Verified spatial volume evaluator

`audio/modulation.rs` and its MXB loader port the volume portion of
`82929F00..8292A450` and construction at `8292B680`, using the same base-disc
executable SHA256 `1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`
and owned MixMap/table hashes recorded above. Nodes retain the first mode's
runtime distance bounds across mode-pointer changes. Curve selector nibble
order is 28, 16, 24, 20; the two weighted products are shifted separately
before addition. Disabled evaluation clears phase; the far-distance branch
retains the published phase. Construction starts at linear 32767/log zero.

The local native-execution verifier compared 12,000 Rust/native updates:
6,000 with the owned collision records and 6,000 with synthetic three-mode
records, including different bounds in later modes, fallback modes, enable
flags, distance boundaries and quadrant boundaries. It executed the original
constructor and full evaluator without behavior hooks, checked bindings and
initial state, and confirmed runtime bounds stayed unchanged. A checked-in
regression test covers the distinct disabled/far phase behavior.

This is an isolated volume stage, not a live playback integration. Auxiliary
pitch and its input-flag mutations remain outside the port. Listener-distance
and directional-phase producers, full graph publication and original voice
positioning must still be integrated before replacing provisional full-scale
collision volume. No ragdoll attenuation was added. (Since replaced; see
[Live collision volume controls](collision-volume.md#live-collision-volume-controls).)

### Collision position and listener-distance producers

`tools/owned_game/ppc_vmx.py` now decodes the VMX128 subset needed by this trace
before falling back to ordinary PowerPC decoding. The instruction encodings
and extended register fields were checked against Xenia's
[instruction layout](https://github.com/xenia-project/xenia/blob/master/src/xenia/cpu/ppc/ppc_instr.h)
and [opcode table](https://github.com/xenia-project/xenia/blob/master/src/xenia/cpu/ppc/ppc_opcode_table_gen.cc).
It is a partial decoder, not an emulator; unsupported instructions return no
result. Destructive arithmetic still has an implicit destination input, so
the displayed register list alone must not be read as an arithmetic formula.

The collision voice's activation method `824BFC58..824BFC9C` obtains the queued
record from owner `+20 -> +44`, stores it at voice `+24`, and passes **record
`+10`** to its linked position controller's virtual `+38`. In the recovered
position-controller table, this is `82557328`, which stores the pointer at
controller `+20`. This completes the position-pointer link from the collision
packet to the spatial producer; it is the queued impact position, not a
replacement sampled from the current player position.

`8249C988` copies position vectors from the listener object rooted at
`*(830734F4)`: root `+00` to controller `+40`, and root `+40` to controller
`+50` (`8249CA9C..8249CAAC`). The calculation in `8249C540` then publishes:

| Input word | Calculation | Evidence |
| --- | --- | --- |
| 1 (float bits) | XYZ distance between controller `+40` and `*controller[+20]` | `8249C6E0..8249C764` |
| 0 (float bits) | XYZ distance between controller `+50` and the same queued position | `8249C7D4..8249C85C` |
| 3 | First directional-phase result from `8249BFD8` | `8249C690..8249C6C0` |
| 2 | Second directional-phase result from `8249BFD8` | `8249C7A0..8249C7D0` |

The distance operation is `vsubfp128` followed by `vmsum3fp128`: W does not
participate. The squared distance goes through `vrsqrtefp128`, two Newton
refinements, and a multiply by squared distance. A zero-distance comparison
selects zero instead of the estimate-derived result. No unit conversion or
additional position scale appears in this distance stage. A host `sqrt` would
be a numerical adaptation, not proof of hardware estimate/rounding parity.

Listener update `8247AA48` is called before family updates in the normal
manager path (`82473230..82473278`). It copies the first listener position to
root `+10`, then obtains a transform via `8277D780` from global context
`*(83027DA0) + 29D20`; transform `+30` becomes root `+00`. A second transform
lookup supplies the direction at root `+20`, normalized with the same
reciprocal-square-root refinement pattern. The second listener updates only
when both unsigned words at context `+29070 +0C` and `+08` are nonzero
(`8247ABAC..8247ABF4`): the old root `+40` is copied to `+50`, then position and
two further vectors are copied from the pointer at context `+29070 +08`.
When that condition fails, those fields retain their previous values. Naming
these sources as a specific host camera/player component still needs a verified
mapping; the address-level producer is established.

Position reset method `8249CAC8` clears controller `+20` and `+1C`. The already
traced publication path subsequently writes the inactive inputs and clears
the input enable bit. Collision-group reuse now has the reset chain traced
below; complete teardown timing remains unresolved. The directional helper uses projected vectors and
an angle helper (`82441210`); its complete numerical behavior is still outside
the implemented scope. No runtime attenuation has been replaced yet. (Since replaced; see
[Live collision volume controls](collision-volume.md#live-collision-volume-controls).)

Further tracing of the first listener's transform accessor `8277D780..8277D8DC`
establishes ordered source selection within the object at context `+29D20`:

| Nonzero byte tested, in order | Selected 64-byte transform offset |
| --- | --- |
| `+1220` | `+1230` |
| `+10D0` | `+1090` |
| `+0F52` | `+0F60` |
| `+1080` | `+1000` |

If all four bytes are zero, accessor `82654C18..82654C94` locks the object
at `+08` through `82F71EAC`. When byte `+0F50` is zero and word `+0E40` is
nonzero, it selects `object + 40 + word[+0E64] * E0`; otherwise it selects
`object + E70`. It unlocks through `82F71EBC` before returning the pointer.
The outer accessor copies all four 16-byte vectors to its destination and
returns that destination. No scale conversion occurs in this selection/copy.
The local native verifier executed both accessors for 243 combinations of
zero/nonzero cache flags and fallback conditions, checked all copied bytes,
the returned pointer, and lock/unlock ordering. Only the synchronization
calls were hooked; selection and copying ran from original executable bytes.
This uses the same base-disc executable hash recorded above. The following
writer trace narrows the cache-state gap; mapping to host components remains
unverified.

Constructor `8277C5E8` clears all four selection bytes at `8277C69C`,
`8277C6EC`, `8277C720` and `8277C764`, clears `+0E40`, and initializes
`+0E64` to -1. The highest-priority setter `8277D9A0..8277D9FC` tests the
supplied object's byte `+10`. Nonzero enables destination `+1220` with byte
1, copies the supplied four-vector transform from `+20` to destination
`+1230`, and copies float `+60` to destination `+1270`. Zero clears only
the enable byte: cached transform and float retain their previous values.
The native verifier checked 1,000 enabled/disabled updates without hooks,
including precedence over all three lower selection flags. No direct branch
caller or literal function pointer was found in the mapped executable;
this setter's invocation/lifetime therefore remains unresolved, and it must
not be assumed to run every gameplay frame.

Another writer at `824EAD44..824EAD90`, conditional on its source byte
`+27A`, targets the verified global-context object at `+29D20`. It writes
selection byte `+10D0` from register r5, copies four vectors from source
`+280` to target `+1090`, and copies source float `+2C0` to target `+10D4`.
Register r5 is the explicit constant 1 at `824EACF0`. This writer belongs to
`824EACA8`, which first atomically reads source word `+10` and returns when
zero, then returns if global object `*(83027DC4) +100 +CD4` is already set.
Otherwise it marks that global byte active and publishes source `+0C` into
the corresponding global pointer slot `+CD8` before applying the overrides.
Source ownership, deactivation and per-frame invocation remain untraced;
this does not establish a host camera mapping. An immediate-offset search found
additional `+0F52` writers throughout gameplay, so a single writer is not
sufficient to establish that flag's state lifetime.

The second-listener condition was corrected after rechecking instructions
`8247ABB0` and `8247ABBC`: both are `cmplwi`, not signed comparisons. Thus
nonzero words, including values with the high bit set, enter the update path.
This is research evidence only; no live listener mapping has been substituted.

Five decoder tests cover extended-register boundaries (512 combinations),
address registers, signed immediates, compare-record bits, permutations and
unsupported words. All 60 focused tests pass. Reuse/efficiency review found no
worthwhile changes; decoder masks and dispatch were also reviewed locally.

### Directional phase and authored modulation-level evaluation

Further VMX decoding establishes the directional helper's packing at
`8249BFD8..8249C248`. The permutation constant at `822F6370` projects the
source vectors into X/Z components. The helper returns zero if either
projected length is below the float at `8209BE30` (approximately 0.0001).
Otherwise it normalizes both projected vectors, clamps their dot product to
`[-1,1]`, and calls angle helper `82441210`. The angle is multiplied first by
float 65535 and then by float `0.15915493667125702`, with a truncating integer
conversion. If the oriented cross term is positive, the result is replaced by
`65535 - result`. The output is therefore an oriented packed turn, not degrees.

The angle helper is a VMX polynomial/estimate implementation, not a call to host
`acos`. It uses coefficients at `822F4480..822F44B0` and the `3F800001` guard
at `822FABB0`, with fused operations and a reciprocal-square-root refinement.
The decoder now includes the vector shift that constructs its sign-bit mask.
This establishes the operation structure; a complete numerical phase port,
including hardware estimate behavior, is still unimplemented.

The **level portion** of the modulation evaluator is now implemented as an
offline reference in `tools/owned_game/mixmap_modulation.py`, from explicit
selected distance, packed phase, authored bounds and owned lookup tables.
The new CLI reads the selected authored node directly:

```bash
python3 -m tools.audio.check_mixmap_modulation \
  --mixmap /path/to/MixMapSK8.mxb \
  --curve-table /path/to/mixmap-curve-table.bin \
  --log-table /path/to/mixmap-log-table.bin \
  --group 3 --node 0 --mode-index 0 --distance 5 --phase 0
```

These distance and phase values are explicit reference inputs. The CLI does
not choose a live mode, recover runtime bounds after state transitions, or
implement the auxiliary pitch output. It rejects nonfinite values, degenerate
bounds and inputs outside the verified normalized domain instead of inventing
native fallback behavior. Constants selected by a mode must be supplied as
those constants; input-buffer slot numbers are not interchangeable with the
selected values.

At `8292A0B8..8292A25C`, the phase's top two bits select bound pair `q` and
`(q+1)&3`; the curve-selector nibbles for quadrants 0/1/2/3 are at shifts
**28/16/24/20**. If distance exceeds both upper bounds, level output is silent.
Otherwise each distance is clamped and normalized with f32 subtraction,
division and multiplication by 32767, then truncated. With phase remainder
`r`, the two curve levels combine as:

```text
linear = (curve1 * (32767 - 2*r) >> 15) + (curve2 * (2*r) >> 15)
```

Both products are shifted separately. The second curve is evaluated only when
`r != 0`. Disabled nodes produce linear zero and log -10000 for this level pair.

`tools/owned_game/mixmap_curve.py` implements all ten selectors at
`82923F18..82924100`. The interpolation weight is exactly
`0x3FF | ((input << 9) & 0x3C00)`; it deliberately ignores input bits that a
generic interpolation would use. The body's selector 6 reverses/complements
the lookup, interpolates, then squares and shifts by 15. The required owned
table spans 513 readable big-endian words from `82FBC598` (2052 bytes), SHA-256
`be693b485c8b528a278c1105000863624b21ffca877cfb2bc52c62b8684e905b`.
A standalone checker is available as `python3 -m tools.audio.check_mixmap_curve`.

The linear-to-log stage at `8292A264..8292A450` uses a separate 512-word table
at `82FBBD98` (2048 bytes), SHA-256
`86bdf6292119e406a7f06df1932cbfc69d2321c0cf4340cb51bf1ba80c18934f`.
It subtracts 602 per exponent step and uses upper-end table indices in sparse
low-level bins. It must not be replaced by a host logarithm. Both tables remain
local. Their offsets in the same mapped base-disc image are virtual address
minus `82000000`; hashes/version constraints from the preceding sections apply.

With the owned files, node 0 at explicit distance 5 and phase 0 yields linear
25101 and log -232. This is a reference result, not a live-game capture. A
local instruction-subset runner executed the actual owned curve helper and
matched 37,439 comparisons, including every one of the 32,768 normalized
inputs for body curve 6. Ten synthetic curve/node tests cover bit masks,
negative-delta rounding, quadrant ordering, blend rounding, logarithmic bins
and malformed input; the complete focused suite passes 71 tests. Review found
no required changes to the recovered curve/node arithmetic.

The remaining volume dependencies are live phase generation, shared sum/control
producers, state lifetime and playback integration. Auxiliary pitch is also
unported. This reference evaluator does not change audible gameplay yet.

## Body cooldown input: publication-header producer

The body timer's snapshot +DC now has a traced immediate writer, distinct
from the MixMap millisecond accumulator. This is additional base-disc evidence,
not a completed host-clock mapping.

At `8249ECE8..8249ECF4`, the audio snapshot publisher forms
`header = *(83027DA0) + 29070`. `8249EE30..8249EE34` copies `header[+00]`
into snapshot +DC. This is the same header read by `SFXObj_HOM` and the
master object's related controls. It is not the f1 delta passed to that
snapshot update.

The header belongs to a publication component at context +29050. Construction
at `82779ABC..82779B5C` installs primary vtable `8230B62C`, secondary interface
`8230B634` at component +4, stores the context's +64 interface at component +8,
and initializes header +00 (component +20) to binary32 1. It registers the
component through that interface's virtual +3C with channel argument 4 and
size argument `1944`.

The secondary callback at `82784518` selects a reader from its r6 argument:
1 calls `827819D8`, 2 calls `827810C0`, and other values return without either.
It adjusts its secondary-interface pointer by +1C, yielding the header at
component +20. For callback kind 1, `827819E8..82781A00` reads a float from
its r4 descriptor +2C, takes its absolute value, and stores it directly in
header +00; the sign bit of descriptor word +30 is stored separately at
header +04. The f1 argument is retained separately at `82781A14`, after the
header value has already been written. Thus this publication field is not
an elapsed-seconds value inferred from f1. The descriptor's upstream writer
and ordinary provider are traced below; callback kind 2 and changes of provider
still require complete tracing before assigning host semantics in every mode.

Combining this producer with the previously recovered body loop establishes:
for a positive region timer, the decrement is
`min(abs(descriptor[+2C]), 1.0)` after callback-kind-1 publication, using the
published value retained in the audio snapshot. The subtraction is binary32,
has no clamp to zero, and occurs after the region's event handling, including
when that handling just assigned the timer. Timer initialization to 15 must
therefore not become a guessed 15-second or 15-millisecond host cooldown.
The number and ordering of audio consumer updates relative to publications
remain part of the required adapter evidence.

The current Rust playback path still uses `impact::Onset`'s 0.12-second host
cooldown and solver-impulse thresholds. The recovered material classifier and
physics-publication strengths remain diagnostic inputs until the full event
scheduler and voice controls are connected. This mismatch is still open;
this trace identifies a concrete producer dependency for replacing it.

## PlayerPhysics speed graph on body strengths

The body loop does not read the published contact strengths directly. The
snapshot publisher first multiplies all eight by a graph of the skater's
centre-of-mass speed, so the classifier sees up to five times the published
value. Addresses are base-disc `default.xex` SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f` (mapped
image `default.pe` SHA-256
`ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42`).

Traced code:

- The snapshot is the Player group's `SFXCTL_PlayerPhysics` controller
  (vtable `822F7CE4`, descriptor `82FD11D0`: family 1, ID 0), registered by
  `8247AF00..8247AF04`. Group binding `828B7858` hands each object the
  controllers it requests. Contacts (`822F71A8`) requests ID 0 (`824DEF58`)
  and keeps it at Contacts `+20` (`824DDC88`). The body loop reads `+1F0`
  through that pointer (`824AA1AC..824AA1B4`).
- Group update `828B7C58` calls virtual `+24` and `+30` of each controller
  on list `+20`, then of each object on list `+24`. The publication
  (`822F7CE4 +24`, `8249ECA0`) therefore precedes Contacts
  (`822F71A8 +24`, `824A60B0`) in the same input update.
- `8249EE28..8249EE2C` stores the player record's `+6C` in snapshot `+D4`.
  `8249F178..8249F198` copies the 0x74-byte block at record `+140` to `+1F0`;
  nothing reads `+1F0..+20F` before the loop. `8249F794..8249F7CC` runs eight
  times: it loads the strength at `+1F0 + 4i` and the float at `+D8`, calls
  `8246FD40` with `r3 = 8`, `r4 = layout + 0x10` and `r5 = layout + 0x30`,
  where layout is `[[[830734C4] +24] +4]`, and stores the `fmuls` product
  (`8249F7C0`) back. Then `8249F7D0..8249F7D4` copies `+D4` to `+D8`.
  The graph thus reads the previous publication's speed, and no clamp
  follows the product. The strength producer clamps to [0, 1], so authored
  bounds above 1, such as the bone and face bands' upper bound 1.25 and the
  `aud_speech/default` head threshold 1.5, take effect only through this
  scaling.
- `[[830734C4] +24]` is the `aud_collisions/default` handle; the body loop's
  cooldown lookup uses the same one. The layout's only field is
  `Hash_8B164823E008749C`, a `Sk8::PointNegGraphData8` of 80 bytes at offset
  0. This path does not read its four header words (0, 1, 1, 5); x and y
  follow at `+10` and `+30`:

  | Point | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | x, speed | 0 | 0.3664494 | 0.4478826 | 0.5211726 | 0.592834 | 0.7410426 | 0.863192 | 0.946254 |
  | y, gain | 1 | 1.2 | 1.485714 | 1.914286 | 2.428571 | 3.6 | 4.571427 | 5 |

- `8246FD40` is the shared point-graph evaluator. An input ordered below
  x[0] returns y[0]. An input not below the last x, including an unordered
  one, returns the last y. Otherwise the first x[i] above the input selects
  the piece; a width that is not positive, or is unordered (`ble` at
  `8246FDC4`), returns y[i], and `fmadds` interpolates otherwise.
- The speed is packet `+6C`. Packet builder `8277E688` writes it at
  `8277E898..8277E8F0` as the three-lane length of PhysOut_SystemReckoning
  `+10` (`vmsum3fp`, `vrsqrtefp` with two Newton steps times the square, zero
  for a zero square, sign cleared). `82BB96AC..82BB96BC` copies that vector
  from skeleton `+3F30`, which `82BB18D8` sets each physics update to
  (COM − previous COM) × the refined reciprocal of the step time. It is the
  centre-of-mass velocity in world units per second. The channel-4 reader
  copies `+6C` into the player record without blending
  (`82781294..827812A0`).
- Initializer `8249E968` zeroes `+D4` (`8249E998`) and `+D8` (`8249EA68`).
  The constructor `8249E900` and PlayerPhysics reset `+2C` (`8249EC98`, a
  branch to `8249E968`) run it. The Player group reset `824E6A78` reaches
  the reset through base reset `828B7E60`. The group update `824E6998`
  resets before publishing when the group's record index is out of range,
  or when a non-local record (`+98` bit 31 clear) fails `824E6AE0`. After
  updating, the Player manager `824DFB08` resets every active group when the
  local record changes, and resets a group whose `+4C`/`+50` key no longer
  matches its record. The first publication after a reset uses
  graph(0) = 1.

TU3 corroboration: upstream research on Title Update 3, through the
skate3recomp recompilation, finds the same block, graph data, one-call lag
and missing clamp. Its TU3 addresses are bridge `sub_824B0DA8`, evaluator
`sub_82481E10`, record `[[0x830CFDA4] +36] +4` and speed writer
`sub_827A1B78`. A recompilation hook measured the body classifier's input
at 5.0 times the clamped region strength (median over 218 queries). That is
TU3 evidence, not a base-disc trace.

Implemented: the exporter writes the x and y arrays to `collisions.json`
version 2 as `settings.speed_graph`. `audio::player_physics::PlayerPhysics`
runs at the start of each Player input phase, before the body and deck
loops: it evaluates `PointGraph` at the stored speed, multiplies all eight
strengths and then stores the current speed. `com_speed` repeats the packet
`+6C` length on `SkeletonBoardFrames::com_velocity`. `PointGraph` now also
takes the unordered-width branch; its former `width <= 0.0` test missed it.
The foot regions 6 and 7 are scaled too, but the port's feet still use host
impulses, so nothing reads them. The `SKATE_AUDIO_BODY_TRACE` and
bail-test traces record the unscaled strengths and `com_speed`.

Verified by replay (local, not checked in):

- `verify_speed_graph.py` ran `8246FD40` for 4,504 evaluations: the owned
  graph and 80 synthetic graphs, with repeated and unordered keys, keys ±1
  ulp, infinite and NaN inputs. All matched `PointGraph`; the former test
  differed in 17 unordered-width cases. The owned graph gives 1.1364446 at
  0.25, 1.7904768 at 0.5, 4.9999995 just below the last key and 5 from it.
- The same script ran `8249EE28..8249EE2C`, the block copy with its native
  `memcpy`, and the loop `8249F794..8249F7D4` for 3,000 publications with
  63 native resets through `8249E968`. Every product and `+D4`/`+D8` matched
  `PlayerPhysics::publish`; the block tail was copied unscaled.
- `verify_bail_speed_graph.py` replayed both clock phases of the stock bail
  trace (484 input updates each) through the native publication and
  `824AA020`, and through the production Rust. Both gave the same 13 records
  per phase. Without the graph the same inputs give 12. With it, in phase 0
  a torso hit at tick 172 reaches category 2, adding a medium overlay, and a
  leg hit rises from category 0 to 1. The bursts at ticks 166 and 172 read
  the speeds of the preceding input updates, 1.75 and 1.84, above the last
  key, so both scaled by 5; the hits at ticks 204 and 212 scaled by about
  1.07 and 1.02.

Provisional in the port: the speed comes from the current physics
publication, not the selected channel-4 packet. `+D8` lasts as long as the
host's body-audio state, because the host has no producer for the reset
triggers above. Which gameplay events (respawn, map change, player
selection) raise them is untraced. Not checked: a bail on a real map, by
ear or against original-game traces.

## Runtime primary-body event bank

The runtime now loads `body_events` and `surface_classes` from the exported
manifest. It requires all four primary body materials, complete 3-by-3 event
matrices, the `Skate_Collisions` bank, and 95 class values in 0..2. Event zero
is valid. Clips share the existing bounded loader, decoded-audio cache and
32 MiB budget; if an event route has no loadable clip, the body-event bank is
unavailable. Older manifests still load their existing representative routes.

With `SKATE_AUDIO_BODY_TRACE`, `body_event_candidates` resolves each of the six
published regions independently using its own raw material and evaluated
intensity. Both the primary and hard-to-medium overlay report the authored
event ID and integer level. This composes the recovered base-disc event matrix
(82484638..824849C4), counterpart selector and existing body classifier; it does
not treat the merged host impulse channels as original body regions.

An on-disk synthetic bank test checks clip reuse, event zero, separate arm
counterparts, below-threshold silence and the overlay event/level. Validation
tests reject missing parts, incomplete matrices, invalid classes and wrong
banks. All 39 skating-audio tests passed. Simplify review removed duplicate
body-level evaluation in tracing.

This remains pre-scheduler diagnostic resolution. It does not queue voices or
change audible playback. The silent-ragdoll report remains open; original
timing, reset/lifetime gates and live controller integration are still required
before replacing the provisional playback path.

## Original body-impact record generation

`audio::body_impact::BodyImpacts` ports the Contacts body loop, base-disc
`824AA020..824AAA34`, with helpers `824AAA38`, `824AAB90`, `824AAD48` and
`824ADA08`. It also ports the ordinary material routines `82484EC8`
(intensity), `82484A98` (level) and `82484D90` (hard-layer scale) for every
material ID. Addresses use the base-disc `default.xex` hash recorded above.

Per call, the loop:

- Writes Contacts controller input word 7 = 0. Word 8 is the channel-5 object's
  float `+18` × 32767, clamped to 0..32767, when its byte `+10` is set, and 0
  otherwise. Context `+29CB4` points to that object, built by `82786308` for
  publication channel 5.
- Latches a rising edge of that byte at `+1A4`.
- Returns without touching any timer when snapshot `+2A4` and `+2A5` are both
  set. These are player packet `+94` bits 5 and 4, copied by `8277EDD8..8277EE08`
  from `PhysOut_State +3B` and `PhysOut_Skeleton +257`. Allocation routine
  `82DB9D98` names the publication table members: `+14` Skeleton, `+18`
  Collision, `+1C` State and `+28` Audio, whose `+5C` is the body contact block.
  `82D8C804` copies processed input `+2468` bit 18 to `PhysOut_State +3B`; the
  Rust port sets that bit for the `Wipeout` attribute and external impulses.
  `82D13F58` copies wipeout physics byte `+1E0` to `PhysOut_Skeleton +257`.
  `82D13DA0` sets that byte only when skeleton hips and neck positional
  speeds are below authored limits, a wipeout timer condition holds and `82D13658`
  returns zero. The loop therefore stops only for a settled wipeout. The
  settled flag now comes from the existing physics output producer (see below).

Each region from head to right leg (snapshot `+1F0`) proceeds only when its
strength is above zero and its timer is not positive. The region selects a
primary body material (head `0x61`, torso `0x62`, arms `0x64`, legs `0x63`).
It also selects cloth (cotton `0x6D` for the torso, skin `0x6B` for arms, denim
`0x6C` for legs). The bone or face layer is bonecrack `0x6E` for the head and
torso, bonesnap `0x6F` for limbs, or facehit `0x70` for the head when snapshot
`+251` is set. A Hall of Meat layer (`0x66`..`0x6A`) exists only while manager
byte `+34B` is set. The surface is the region contact tag (snapshot `+230`)
minus one. Tags outside 1..0x90 select the absent material `0x8F`.

Both sides of the pair are classified. If either is audible, the loop:

1. Sets the timer to `aud_collisions/default` `Hash_6DD85F43C1B6E6AA`
   (Int32 15), converted to float.
2. Enqueues the primary pair.
3. Enqueues a medium layer for each hard side, scaling that side's hard level
   by its hard-layer scale.
4. Enqueues the cloth layer, using the soft endpoints over the primary body's
   own band.
5. Enqueues the bone or face layer if its band applies.
6. Enqueues the Hall of Meat layer when `824ADA78` allows it.
7. On a pending channel-5 edge, enqueues a torso layer if the region is not
   the torso and the body category is at least 1. Any first audible region
   sets controller word 7 to 32767 and clears the edge.

After a region, a positive timer loses `min(snapshot +DC, 1)` with no zero
clamp, so a new timer is decremented in the same call. The constructor
`824A5B70` zeroes the timers and latches; nothing else resets them.

`824A9CD8` loads the bone and face bands from `aud_collisions/default`:

| Contacts field | Use | Owned value |
| --- | --- | --- |
| `+128`/`+12C` | Torso bonecrack, category 1 | 0.7 / 1.25 |
| `+130`/`+134` | Head bonecrack, category 1 | 0.75 / 1.25 |
| `+138`/`+13C` | Bonesnap, category 1 | 0.65 / 1.25 |
| `+140`/`+144` | Soft facehit, category 0 | 0.05 / 0.3 |
| `+148`/`+14C` | Medium facehit, category 1 | 0.3 / 0.55 |

Lower bounds are strict. A category-1 bone or face layer above its upper bound
is dropped, with no fall back to the soft band. Head strength above
`aud_speech/default` 1.5, and snapshot `+250`/`+254`/`+258`/`+260`, only drive
speech (`824AB928`, `824AD490`). Neither of those routines writes this state or
enqueues collisions.

Snapshot `+250`/`+251` are SkeletonState `+FA8`/`+FA9`, the current flags of
the two specific contacts. Base-disc `82BAD960` stores part 23 (groin) at
`+F50` and part 1 (face) at `+F54`. `82BAC9D8..82BACA40` sets the current and
recent flags, and `82BAC678` clears current each update. This matches
`SpecificContact` in the Rust skeleton, so facehit uses the face flag. The
0x74-byte block copied at `8249F198` from packet `+140` holds the strengths,
region floats, contact tags, these flags, and the speech floats.

Material IDs index table `82FD1930`. Word `+00` is the bank: 0
(`Skate_Collisions`), 1 (`Skate_Metal`) or 2 (Hall of Meat). The 64-bit key at
`+08` names the `aud_material` row; 142 of the 143 entries resolve in the owned
database, and `0x5E` is empty. Several IDs share a row: tag ID 8 is
`default_metal`, and the snow and ice IDs `0x45..0x47` are `grass`. The
exporter's collision-tag `SURFACES` names are therefore not interchangeable
with these audio material IDs.

PowerPC `ble`/`bge` branch on unordered compares, so the classifier treats an
unordered strength as soft. The level routine returns the upper endpoint for
unordered bounds. The port mirrors these branches; the generator itself rejects
nonfinite frames.

`verify_body_impact.py` (local) runs the owned `824AA020` together with
`824AAA38`, `824AAB90`, `824AAD48`, `824ADA08`, `82484EC8`, `82484A98` and
`82484D90` in a new instruction-subset interpreter (`ppc_interp.py`). It hooks
only database lookups, the verified counterpart class, attribute reads, speech,
`824ADA78` and the enqueue and controller calls. The production Rust ran on the
same sequential frames. All records matched in content, order and flag bytes,
as did the final controller words and cooldown bit patterns:

- Owned profiles: 2,400 frames, 1,813 records.
- Synthetic profiles: 1,800 frames, 10,830 records. These include unordered
  and degenerate bounds, zero and negative cooldowns, and arbitrary bands; IDs
  that share an `aud_material` row share a profile.

Mutating the cooldown value, the Hall of Meat predicate or the flag byte order
each failed the replay. Six Rust unit tests cover the same rules.

Downstream, the consumer `824BFE08` handles both sides of a record. It skips
material `0x8F` and category 3 and resolves bank, event and controls through
`82484410`. Record byte `+28` is that routine's mode argument (special pitch
override `Hash_3A3DD47E8DAFE796` when `aud_material` `+38` is set). Byte `+2A`
enables a bank-8 replacement for body materials while manager byte `+224` and
a further lookup are set; torso also carries `DLC_Cartoon_Collisions`
`Hash_B15007D53511CC46`. Event fields by bank (`82484638..824849C4`):

| Bank | Soft, classes 0/1/2 | Medium, classes 0/1/2 | Hard |
| --- | --- | --- | --- |
| 0 | `+40` `BFAB…`, `+6C` `9ABF…`, `+78` `BCD5…` | `+3C` `EF9B…`, `+68` `A3AD…`, `+74` `C676…` | `+70` `9203…` |
| 1 | `+50` `66A9…`, `+58` `5955…`, `+64` `79DD…` | `+4C` `B722…`, `+54` `1411…`, `+60` `5079…` | `+5C` `F542…` |
| 2 | `137C…` (`82470500`), `2A28…`, `79BD…` | `5432…`, `3FCB…`, `5331…` | `3EA2…` |

Bank 2 reads those fields by hash. The owned `hom_*` rows type them as
`HOM_Set_1`. The manager's 10 collision groups are chosen by `824DF400`: the
first group with active byte `+34` clear, in creation order. Otherwise the
group with the smallest `+40` is reset and reused; ties go to the earlier
group. `824E6728` stamps `+40` from publication source `*(830734B4) +10` when a
record is accepted. While active, `SFXObj_Collision` writes its controller
word 0 = 32767 and word 1 = 10000, 20000 or 32767 for the record's highest
audible category (`824BFCA0`).

The collision-input publication is now ported as
`audio::collision_group::publish_inputs`, preserving the original setter
call order: clear word 0, clear word 1, then (only for active group byte
`+34` and either nonnull voice pointer `+28`/`+48`) write word 0 = 32767
and word 1 = intensity. Record signed words `+08`/`+0C` choose that intensity:
if the first is category 3 use the second; otherwise if the second is 3 keep
the first; otherwise take the signed maximum. Selected category 1 maps to
20000, category 2 to 32767, all others to 10000. Even two category-3 words
map to 10000 if a channel exists; the channel gate must remain separate.
A null binding skips setters. These are full input words for controller
`40030000` with original instance bits, not packed output volume halfwords.

Native/Rust execution matched 1,176 combinations of binding presence,
group active bytes, channel pointers and signed category boundaries. The
native virtual setter was hooked to observe calls, so this verifies publication
values and ordering rather than its underlying buffer implementation. Two
checked-in tests cover clear/set ordering, missing bindings/channels,
category-3 exclusions and signed comparisons. This uses the same base-disc
hash recorded above. Live graph attachment remains unported; the game still
uses provisional full-scale volume output. (Since replaced; see
[Live collision volume controls](collision-volume.md#live-collision-volume-controls).)

The reuse reset resolves through group vtable `822F8030 +1C` to `824E6518`,
which first invokes base reset `828B7E60`. That clears group `+1C` and active
byte `+34`, then traverses the object list at group `+24` in linked order.
Each object receives virtual `+2C`; collision voice `822F73A0 +2C` is
`824A60A0`, a virtual `+20` dispatch to `824BFB80`, which clears voice record
`+24` and releases its two playback channels. For each object, base reset then
clears the linked output buffer's whole word `+3C` (not just its low bit) and
zeroes the linked input buffer's full 64 bytes. Null bindings/buffers skip
their respective stores.

Next, the controller list at group `+20` receives virtual `+2C` and its input
buffers are zeroed. This second traversal does not clear output words. The
collision position-controller table `822F6B28 +2C` resolves to `8249CAC8`,
clearing its position source `+20` and additional source `+1C`. Finally
`824E6518` frees and clears group record `+44` when present.

Registration fixes the list membership. `8247ACC8` passes object descriptors,
including the collision voice (`82FD13A0`) and Contacts (`82FD1310`), to
`828B7020`, whose registry `828B7788` instantiates onto list `+24`. It passes
controller descriptors, including 3DColPos (`82FD1240`) and PlayerPhysics
(`82FD11D0`), to `828B6F48`, whose registry `828B7AD0` instantiates onto list
`+20`. The local verifier executed 200 resets through the actual
group/position/voice vtables with randomized buffer contents, verified both
64-byte input clears, position-pointer resets and output disable, and used no
hooks. It placed the position controller on `+24` and the voice on `+20`, so
its output-disable check exercised the position controller's binding; the
voice's output clear rests on the disassembly. The port's reset clears the
output word that the group's voices read, which matches the game's layout.
Playback channels and the group's owned record were null in this
verification; release/free internals remain outside that verified domain.
All addresses use the base-disc hash recorded above.

An allocation boundary also matters: `824DF438` initializes its best timestamp
to unsigned `FFFFFFFF` and updates only for a strict smaller timestamp. If
every active group has that exact timestamp, no candidate is selected and
allocation returns null; a generic minimum choosing the first group would
change this boundary. Normal timestamps and equal nonmaximum timestamps
preserve the documented earliest-list-entry tie behavior.

`audio/collision_group.rs` now ports this allocator decision from explicit
active bytes and publication timestamp words in creation order. It returns
the selected index and whether reset must precede reuse, without inventing
host timestamps, state transitions or completion timers. Native/Rust replay
matched 2,004 lists, including empty lists, all-maximum timestamps, high-bit
unsigned ordering, tied ages, noncanonical active bytes and inactive groups.
The native allocator's reset callback was hooked to record its dispatch;
the separate 200-case reset verification above covers the traced reset body.
Two checked-in tests retain allocation boundaries. The game's `Groups::allocate`
now delegates its decision to this core selector from a ten-entry stack
snapshot, removing the duplicate minimum scan. The existing host entity
ownership and stamp publication remain separate adaptations; this change
does not claim original timestamp production or MixMap reset integration.
The game allocation/completion regression passes, including earliest ties,
finished voices, empty playback and the all-maximum timestamp boundary.
Live graph ownership and reset integration remain incomplete.

Normal completion is a playback-status path, not an authored timeout.
`824C01B8` enters only for an active owner group. If both voice channel
pointers `+28` and `+48` are null on entry, it invokes initial playback
`824BFE08` and returns; that entry condition must not be mistaken for
completion. During the existing-channel path, `824C0298..824C02C4` asks
`8294E158` about each nonnull channel, tests the returned low byte, and
releases/clears the pointer when that byte is zero. After both channel
iterations, `824C0440..824C0468` invokes the owner group's virtual `+1C`
reset if both pointers are now null. Thus live integration must retain the
distinction between initial playback and finished playback and cannot
substitute a guessed duration. The playback-status backend's lifecycle is
still unported.

This is the original record producer, verified against owned code. The next
section connects it to playback.

## Runtime body-impact playback

When the impact library includes `collisions.json`, `skating_audio` plays head,
torso, arm and leg impacts from the original body loop and clears the host
impulse channels for those parts. The same bank connects ordinary deck
playback ([details](board-collisions.md#runtime-ordinary-deck-impact-playback)). Trucks and feet
keep the provisional onset path, as do body and deck contacts when no bank
loads. The runtime reads only `collisions.json` version 2, which adds the
speed graph; an older export logs `invalid or outdated collisions.json`.

After each fixed simulation tick, the manager clock (`audio::clock`) advances.
On an input phase, the PlayerPhysics publication runs first, then the loop
reads:

| Snapshot input | Runtime source |
| --- | --- |
| `+1F0` strengths, `+230` tags | Regions 0..5 of the published body contact block (`8274FF60` four-publication maxima, current tags); the strengths times the [speed graph](#playerphysics-speed-graph-on-body-strengths) at the previous input phase's speed |
| `+2A4` | Processed input `+2468` bit 18 |
| `+251` | Current flag of the part-1 (face) specific contact |

Each record takes a group inside the same input phase, as `82474D30` hands it
to `824DF400`: the first inactive group, else the oldest stamp, whose voices
are stopped and whose group is reset. Activation (`824BFC58`) stores the
record and its `+10` position. Initial playback follows the group's first
evaluated MixMap phase. A side that is not the absent material or category 3
resolves its bank and event through `82484410` and `82484638`, using the
counterpart's class (0 when the counterpart is absent). Its voice plays the
event's rendered clip:

- Gain is `824C0334..824C03DC` applied to the record level, base level and
  the group's material-class volume slot, times the clip's render trim and
  `SKATE3_AUDIO_VOLUME`. It is recomputed every rendered frame from the
  group's live controls ([details](collision-volume.md#live-collision-volume-controls)).
- Speed combines authored pitch with the group's live pitch controller
  (slot 1, or 22 for class 9) through the original consumer
  ([details](collision-pan.md#collision-pitch-controllers)); it is about −4 cents at normal speed.

The consumer has no gain gate, so a zero-gain voice still holds its group. A
group becomes free when its voices finish (`824BFD14`). Collision voices do not
count against the host's eight-voice budget.

These inputs have no ported producer and are provisional:

| Input | Runtime value | Missing evidence |
| --- | --- | --- |
| Manager clock delta | Fixed tick period, enabled, never forced. At 60 Hz the loop runs every second tick, so the 15-update cooldown lasts 0.5 s | Original measured loop and enable initialization traced; host packet-buffer adapter and force producer remain ([details](collision-pan.md#audio-manager-cadence-and-publication-timing)) |
| Snapshot `+DC` | 1.0, the provider's constructed ratio | Exhaustive `+20` writer audit and selected channel-6 packet binding; skipped advances retain `+24` ([details](collision-pan.md#audio-manager-cadence-and-publication-timing)) |
| Packet `+6C` speed, snapshot `+D8` | Length of the current physics publication's COM velocity; `+D8` starts at 0 and is never reset | Selected channel-4 packet binding; the gameplay events behind the Player group reset triggers ([details](#playerphysics-speed-graph-on-body-strengths)) |
| Manager `+34B`, `824ADA78` | Off, so no Hall of Meat layer | Hall of Meat mode producer |
| Channel-5 object (`+29CB4`) | Duration-gated Collision `+C4` strength now feeds word 8 and the torso edge layer ([details](collision-pan.md#channel-5-live-duration-publication)) | Material/tag State `+41/+45` routes and driver teleport-input mapping ([suppression audit](collision-pan.md#channel-5-suppression-writers)); selected packet index; section A is copied, not fraction-interpolated ([details](collision-pan.md#audio-manager-cadence-and-publication-timing)) |
| Contacts `+1C` owner | Byte 0, word nonzero | Owner object binding |
| Volume controller slots 12..21 | Live MixMap controls (`mixmap/` is required) | See [Live collision volume controls](collision-volume.md#live-collision-volume-controls) |
| Pitch controller slots 1 and 22 | Live bound MixMap outputs; normal-speed values −4 and 0 cents after startup | Original Rsp0 smoothing/interpolation and outer publication cadence remain provisional; see [Collision pitch controllers](collision-pan.md#collision-pitch-controllers) |
| Group stamp | Host tick counter | Original mask-bit-0 increment/reset traced; selected channel-6 packet binding remains ([details](collision-pan.md#audio-manager-cadence-and-publication-timing)) |
| AEMS event/layer playback | Premixed variants, evaluated start gain | Bank choices/delays/gains and Gai0 now traced; queue-to-render ordering, source cursor and other dependencies remain ([research and plan](collision-pan.md#aems-collision-event-start-choices-layers-and-timing)) |

Host adaptations, which make no parity claim:

- At most 64 records wait between frames; the original queue capacity is
  untraced.
- Voices are not positioned; record `+10` is unused.
- Each event rotates through up to four pre-mixed layer variants instead of the
  original randomizer, timing and envelopes.
- Record `+28` special pitch, the `+2A` bank-8 replacement and Hall of Meat set
  overrides are not applied.
- Pause, replay, map loading, teleports, mod vehicles and mute stop collision
  voices and clear queued records and groups. They keep the body timers, which
  only constructor `824A5B70` resets, and PlayerPhysics `+D8`. Whether a map
  load reconstructs the Contacts or PlayerPhysics object is untraced.

The empty material `0x5E` (tag `0x5F`) is not reproduced. The native
classifier does not fail on it: the missing database row leaves the handle on
static object `83073F70`, which is zero in the image, and the bounds are read
through it. That object's runtime contents are untraced, so the port rejects
such an update and logs once, rather than assuming zero bounds. The owned
collision-tag enum ends before `0x5F`.

Validation:

- Rust tests cover group allocation, stealing and release, and side
  resolution with counterpart classes. An opt-in test loads the owned export,
  resolves all 280 events, checks the speed graph, and checks that every
  audible side of the records generated for each tag at seven strengths, up
  to the graph's 5, has a clip:

  ```sh
  SKATE_OWNED_AUDIO_ROOT="$PWD/.local/collision-export-validation" \
    cargo test -p skate-game skating_audio::collisions -- --include-ignored
  ```

- The stock controller bail test writes each tick's loop inputs when asked:

  ```sh
  SKATE3_ASSET_ROOT="$PWD/data/$(plutil -extract directory raw -o - data/installation.json)/assets" \
  SKATE_BODY_AUDIO_TRACE=/path/to/bail.json \
    cargo test -p skate-game raw_controller_bail_runs_stock_wipeout_for_900_ticks -- --include-ignored
  ```

  The trace holds the strengths before the speed graph and packet `+6C`
  (`com_speed`). The local `verify_bail_speed_graph.py` replays both clock
  phases of that trace (484 input updates each) through the native
  publication and `824AA020`, and through the production Rust. In each phase,
  both produced the same 13 records; without the graph, as in the earlier
  `verify_body_bail.py`, they produce 12. Leg, arm and torso primaries, with
  denim, skin and cotton layers, fall between ticks 166 and 212; state 300
  began at tick 67. The test ground's tag 0 selects the absent surface, so
  only body sides sound there.
- With the export installed, `./play.sh --verify shot.png` logs
  `original collision bank loaded with 280 events; body speed graph x1 at 0
  to x5 from 0.946254` and passes.

Not yet checked: a bail on a real map, by ear and against the
`SKATE_AUDIO_BODY_RECORD` log. The silent-bail report stays open until then.
