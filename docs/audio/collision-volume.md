# Collision volume

The MixMap program, envelopes, shared sums, scalar and curve evaluation, output bindings and the live volume controls. Part of the [audio research](README.md).

## Shared collision-volume control: HOM envelope

Further base-disc tracing (same `default.xex` SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`)
identifies a live producer behind collision output argument `B100304C`.
This is recovered code/data evidence, not an implemented runtime fix or an
original-game capture.

The raw identifier `B100304C` would select family 0, instance 6, envelope
index 76 if passed directly to resolver `82927764`. **Correction:** collision
output binding rewrites this identifier first; the live output does not select
instance 6 from these authored bits. It expands across configured family-0
instances, as detailed below. In the owned MixMap, group 0's +18 section starts
at `0670` and contains 245 records. Record 76 at `0ECC` has source controller
`400000C0` and one multiplier reference, declaration `00000079`. Within each
constructed instance `i`, these become `400000C0 | (i << 11)` and
`00000079 | (i << 11)`, respectively.
Declaration 121 at `0480` reads controller `48000020`; its live publication
and declaration evaluation still require tracing. The reference resolver's
false/true argument selects envelope runtime +1C/+18 respectively: these
are the linear envelope value and its derived logarithmic contribution.
The collision sum consumes the latter.

Registration `82FD1580` identifies object tag `000000C0` as `SFXObj_HOM`,
factory `824DC0E8`, vtable `822F7AA8`, update method `824DC1F8`. Do not infer
all its gameplay meanings from the name. The factory clears its two latch
bytes at +1C/+1D. Let `manager = *(830734E4)` and
`context = *(83027DA0) + 29070`. The recovered update is:

- When manager byte +34B is nonzero and context float +0 is below 1, it
  writes input 0 = 32767 and input 3 = 0 only if manager byte +34C is nonzero
  and latch +1C is clear. This branch leaves the latches unchanged.
- Otherwise within that active condition, it writes input 0 = 0 and input
  3 = 32767, and sets both latches. Outside the active condition, it writes
  both inputs as zero and clears latch +1C.
- Latch +1D clears if context pointer +8 is null or that pointed object's
  word +94 lacks mask `0x20`. Input 4 then reflects this latch as 0/32767.
- Input 1 is 32767 exactly when manager word +49C equals 1. Input 2 is
  32767 exactly when manager word +36C equals 31 and manager byte +1CC is
  nonzero. Other cases publish zero. Each publication is conditional on
  the bound controller pointer being present; latch updates follow the
  original branches even when that pointer is absent.

The input feeding envelope 76 is therefore a state signal, not contact force
or landing height. Its upstream manager/context writers and instance lifetime
remain unverified; these fields must not be mapped to host bail flags by name.

`audio::hom` now ports this immediate publication from explicit original
manager/context fields, preserving write order 0, 3, 4, 1, 2 and both retained
latches. A missing controller suppresses writes without suppressing state
transitions. The local native verifier matched 2,000 sequential updates,
including boundary-adjacent timing ratios, infinities/NaN, null secondary
objects, high-bit flags, missing bindings and changed manager conditions.
Only the controller setter was hooked to capture its calls; the original
state branches executed directly. A checked-in sequence test covers retained
latches through missing bindings and secondary-source removal. This is not
a host bail-state adapter; upstream fields and their lifetime remain missing,
and the game still uses provisional collision volume.

### HOM manager flags and authored front-end state

The immediate manager flag writer is `82476170`, using the same base-disc
executable hash above. It reads the publication record at context `+290B0`.
At `824769F8..82476AAC`, manager `+34B` clears unless manager word `+354`
equals 1 and publication word `+20` contains mask `04000000`. Within that
gate, timing ratio at context `+29070 +00` is compared with the authored
threshold. If ratio is greater and the previous `+34B` is zero, it remains
zero; otherwise it becomes 1. Hence once activated it stays set until the
mode/publication gate clears. Independently, manager `+34C` becomes bit 25
of that publication word (mask `02000000`). Constructor `82472688..8247268C`
initializes both bytes to zero.

Threshold lookup `82476A0C..82476A64` uses class hash
`C1831BDB6CB1B1EA` (`aud_general`), row hash `34049ABCE23C10AC` (`hom`),
and field `Hash_4BD3E0CCEA90BE26`. Hashes were independently checked with
the VLT name-hash implementation. The owned `skatercollections.vlt` SHA256
`3b7dbd062bb1c906a085514355afff35cfa22f486ae70820c5ad1a42a7aab25b`
types the field as Float with bytes `3F800000` (1.0). Missing lookup selects
the shared default word at `83073F70`; its lifetime/default writer has not
been traced. `audio::hom::ManagerFlags` therefore accepts an explicit resolved
threshold and never substitutes 1.0 for absent data. Native/Rust replay matched
2,000 sequential flag updates, including equality, neighboring float values,
NaN/infinities, mode changes and prior latch retention. Authored lookup and
release calls were hooked; the flag branches executed from original bytes.

Manager `+49C` has a separate source: writer `82478850` selects class hash
`1FCCED72C8A5FE81` (`fe_hudstate`) from its incoming object's 64-bit `+10`
key. It reads field `Hash_79948D742323546E` and stores it at `824788D4`.
Owned data types it as `Sk8::Audio::eAudioFEStates`; `default` has value 0,
`homresults` value 1 and `replay2` value 2, with other front-end rows carrying
other enum values. Missing lookup again selects `83073F70`. This is an
authored front-end audio state, not evidence that a physical fall sets it.
Other publication flags, manager `+354`/`+36C` writers and front-end event
dispatch still need full lifetime tracing before these inputs can run live.

### HOM publication bits: actor source

The channel-4 writer interface is primary vtable `8230B62C +04` =
`827844A8`. Kind 1 invokes `8277DCC0`; the secondary reader interface
`8230B634 +04` = `82784518` invokes `827819D8`, which copies the aligned
98-byte publication record into context `+290B0`. The primary/secondary
tables have two entries each; adjacent camera entries must not be treated
as more methods of this publication interface.

Writer `8277DCC0` builds that record with helper `8277F998` at `8277E160`.
It passes the retained selected actor in r4 (`8277E154`). At
`8277FC70..8277FCB8`, a nonnull actor supplies object `actor[+3C]`:
mask `30` in that object's word `+CC` and nonzero byte `+3937` enable
publication bit `04000000`. If enabled and float `+C8` equals zero, it also
sets `02000000`. Both +0 and -0 satisfy the zero comparison; NaN does not.
The preceding packing (`8277FC5C..8277FC6C`) clears those two bits before
these contributions, so absent actors and failed gates do not retain them
from the previous frame.

`audio::hom::publication_bits` now ports just those bit contributions from
explicit raw source fields. Native/Rust replay matched 180 combinations,
including absent actors, each flag bit, noncanonical enable bytes, signed
zeros and NaN/infinities, without hooks; other publication bits remained
unchanged in the native block. The full record packing and actor selection
remain outside this helper. Actor `+3C` field writers/units and correspondence
with host components still need tracing; no live ragdoll flag is substituted.
Addresses use the same base-disc executable hash above.

### Publication actor activity value and initialization

The direct byte writer search identifies initialization at `82DB9990` and
publication at `82D8493C` for source `+3937`. Initialization clears that byte;
the latter writes 1 on completion of update `82D84308`. Its worker's pointer
`+190` supplies the actor, whose `+3C` pointer supplies the same object read
by the publication helper. Immediately before setting the byte, the update
writes float `+C8` at `82D84930`.

The final block `82D8483C..82D8493C` checks 25 signed classification pairs:
five groups spaced `140` bytes, each with five pairs spaced `40` bytes,
starting at worker `+1C0/+1C4`. A pair counts exactly when its first word
is at least 4 and its second word is below 4. Nonzero transition count
sets retained worker word `+7F4` to 30; otherwise a nonzero word decrements
once. The count is also stored at actor `+3C -> +D0`. A nonzero retained word
publishes float 1 at `+C8`; zero publishes float 0. Thus the publication's
alternate bit distinguishes this retained activity value, rather than
testing a host frame delta. The counter counts calls to this update, not
seconds; its enclosing cadence remains untraced.

The update calls `82D838D8` before `82D83AD8`. The former copies each
entry's current classification at `+20` into its previous classification at
`+24`, alongside the current-to-previous vector copy. Therefore the final
condition is **current >= 4 and previous < 4**, not the reverse. Refresh
`82D83AD8` reads the actor's `+18` bone observation storage for 25 entries,
mapping indices 23 to 1, 24 to 23, and other indices to index + 1.
Its scaled observation calls `82D83820`, which adds to entry float `+2C`
and retains the greater of that accumulated value and entry `+28`.
Four authored neighbor links can recursively distribute the observation,
excluding link -1 and the immediate source index. The update's subsequent
classification loop at `82D84414..82D84458` scans indices 5 through 0
of thresholds spaced eight bytes apart, from table `worker +4`, per-entry
stride `54`. It selects the first threshold strictly above zero and strictly
below entry `+28`, or -1 if none qualifies, then decays `+2C` by the
executable's float constant at `8208821C`. These are observation bins;
mapping them to host ragdoll states is not established. The core helper
`audio::hom::observation` now ports this recursive accumulation and descending
classification against explicit authored records. It preserves neighbor order,
immediate-source exclusion, per-edge strict 0.1 gating, retained peaks, and
per-classification float decay. A local oracle executes `82D83820` and
`82D84414..82D8445C` without hooks and compares all 25 peaks, accumulators,
and bins across 1,500 updates using the owned table, including periodic fresh
state fixtures and floats immediately below, equal to, and above authored
thresholds. Fixture clearing does not execute or validate the entire reset
routine. This helper does not implement the complete worker or connect host
ragdoll inputs: upstream graph inputs, full state lifetime, and host publication
alignment remain dependencies before live integration.

Constructor `82D81FE0` calls `82D77A58`, which looks up class hash
`9E4C471BC6EAB615` and row hash `D7EDBD362D7D2152`. Independently hashing
the owned collection names identifies these as `scoring_wipeouts/default`.
Binding helper `82B447B8` puts the collection's `+24` raw-data pointer at
worker `+4`; failed lookup instead uses `83073F70`, whose fallback contents
remain unverified. The owned row's field `Hash_1F9B5EA147CBEEE9`, typed
`Sk8::Score::HoM::BoneSetAttribData`, contains exactly 2,100 bytes:
25 records of `54` bytes, matching the executable's entry stride and count.
Each record contains an integer ID, four `(signed neighbor, float weight)`
pairs, and six `(integer score, float threshold)` pairs. For example,
record 0 has thresholds `[7, 14, 0, 0, 21, 0]`, and record 24 has
`[3, 5, 7, 0, 14, 0]`. These authored values are not audio gain levels.
The owned schema independently places this field at raw offset zero,
with size 2,100 and flags `06`, confirming the live table correspondence.
The executable decay constant `8208821C` is float bits `3F4CCCCD`
(approximately 0.8). Observation scaling at `82D83C98..82D83CAC`
multiplies the selected observation by float 2 (`82060C50`), then by
the collection float at raw offset `BFC`, and accepts it only when strictly
greater than float bits `3DCCCCCD` (`820641A0`, approximately 0.1).
The schema maps `BFC` to field `Hash_B7514DE7EBDE9D01`, size 4,
flags `06`; the owned default row gives float bits `3F400000` (0.75).
The two single-precision multiplies must retain their executable ordering,
rather than being replaced by an assumed host conversion. The observation
producer remains unresolved. This trace identifies another original dependency;
it does not establish a host impact-speed conversion or change playback.
Collection provenance is `skatercollections.vlt`, SHA-256
`3b7dbd062bb1c906a085514355afff35cfa22f486ae70820c5ad1a42a7aab25b`.
Schema provenance is `skaterschema.vlt`, SHA-256
`63b9e652b7918d811ec135915b926e687727b98511a3d47d38a0381a34406b9c`.

The observation pointer is the physical publication table's Collision member,
not a second skeleton state object. Constructor `82DB9D98` allocates `DA0`
bytes at `82DB9F28`, initializes its `+50` storage with `82DB84D0`,
and stores it at table `+18` at `82DB9F58`; the allocation label at
`8206FB5C` is `PhysOut_Collision`. Initialization loops over 24 records,
each `80` bytes, and clears both contact scalar/vector sets and four group
bytes. `82DB85A8` clears the same contact fields when invoked by the
Collision reset `82DB87A4`; its runtime callers still need tracing.

Publisher `82BAD9F0` loads table `+18`, uses its `+50` storage, and copies
24 bone observations as three unrolled batches of eight. Its input starts
at collision-feedback `+520`, advancing `70` bytes per bone and `380`
bytes per batch (`82BAE1C0`). Destination starts at Collision `+F0`,
advancing `80` per bone and `400` per batch (`82BAE1C8`). Stores
`82BADB54` and `82BADB78` copy feedback entry floats `+0` and `+4`
to destination `+24` and `+54`. These are exactly the two scalar sources
read by the wipeout worker's observation selection, with no intermediate
normalization. The publisher also copies the contact tags and group bytes.
The host has corresponding `BoneContact::force` and `specific_force`,
but their existing TU3-labelled producer must be mapped to this base-disc
feedback owner and checked before adapter integration. In particular,
the similarly named `weighted_force` and normalized audio region strength
are independent quantities and cannot substitute for these per-bone values.

Base-disc collision-feedback update `82BAC358` establishes the matching
producer shape independently of the host's TU3 labels. It starts its
24-entry reset at owner `+4F0`; the loop `82BAC578..82BAC5C8` clears
vectors, both floats at entry-relative `+30/+34`, tags and four group bytes,
using stride `70`, while decrementing contact ages by input `+50`.
Thus the first bone's two scalar locations are owner `+520/+524`, exactly
the publisher's source. Native execution of this loop matched 100 randomized
storage fixtures for all 24 cleared contact records and age decrements,
without hooks. This verifies the loop, not the entire collision update.

The contact path computes relative velocity at `82BACC88` by subtracting
the chosen own-velocity vector from the other body's velocity. Its
`vmsum3fp` against the signed report normal at `82BACC98`, followed by
sign-bit removal, supplies float `f31`. The small-object branch multiplies
it by `f26` at `82BACCAC`; group 5 applies the selected authored scalar
at `82BACD18`. Bone writes at `82BACE18`/`82BACE2C` retain this value
only when strictly greater than the existing specific/ordinary value.
The bone entry address is owner + `(part + B) * 70`; the stored floats
are at `+54/+50`, equivalent to owner + `520 + part * 70` plus `4/0`.
This explains why `BoneContact::force` is a contact-relative speed quantity,
despite its name. The separately calculated solver-spy weighted quantity
is not substituted. The native path's full own-velocity selection, VMX
rounding, contact gates, part indexing and update/publication cadence still
need validation before connecting the host fields to the mix worker.

Worker entry `82D83620` adds an important lifetime constraint. It reads
`PhysOut_Filtered` (publication table `+40`, allocation label `8206FC10`,
table store `82DBA2F8`) word `+0`. Worker byte `+8B4` becomes 1 exactly
when that word equals 4 and table `+1C` bytes `+45/+44` are both zero;
otherwise it becomes zero. The prior byte is retained in a register for
transition handling. At `82D837D8..82D83804`, only a nonzero new `+8B4`
dispatches `82D84308` and advances the activity counter; the other path
dispatches `82D83EF8`. These raw conditions cannot yet be replaced by a
host ragdoll boolean. Filtered-word and byte producers remain unresolved.

The same entry forms a second retained byte `+8B5`: filtered words 2 or 7
clear float `+8BC`; otherwise each call adds float bits `3C888889`
(`82084998`, approximately 1/60). The byte is 1 for words 2/7, or while
its previous value is nonzero and `+8BC` is strictly below float bits
`3DCCCCCD` (approximately 0.1). Reset `82D821D8` is called at
`82D83768` when the secondary byte rises, or when the primary byte rises
while the old secondary byte is zero. The latter additionally captures the
publication vector and score inputs at `82D83778..82D83798`. An already
nonzero secondary byte suppresses this primary reset even if the secondary
expires during the current update. This establishes runtime reset ownership
and rules, but not the outer caller's wall-clock cadence.
The `+8BC` increment is a recovered constant, not evidence that the host
may advance this worker once per rendered frame.

On a changed `+8B4` whose new value is nonzero, `82D837B8` calls
`82D82C78` before normal active refresh. On a changed byte whose new value
is zero, `82D837D4` instead calls `82D82D78` after setting selected body
byte `+3936=1`. The branch at `82D837B0` explicitly jumps to the latter
path for zero: earlier notes had these directions reversed. This computes authored graph contributions from
worker `+824/+828`, scales them by collection `+BFC`, and calls the same
recursive accumulator helper `82D83820` for entries 15, 14, 19 and 18.
Entries 14 and 18 additionally use collection `+BF8`, mapped by the owned
schema to `Hash_72A3CCC3C75697CD` (float bits `3ECCCCCD`, approximately
0.4). It then clears `+824/+828`. Thus collision-observation refresh is
not the accumulator's only producer. This additional graph path and its
inputs must be preserved before claiming a complete activity-worker port.

Core `audio::hom::gate::GateState` now ports gate selection, fixed float
elapsed updates, reset/capture conditions and ordered transition actions.
Constructor-zero state is explicit; the scoring reset does not clear these
gate fields. A local native/Rust oracle compares 2,000 fixtures, including
raw nonboolean prior bytes, suppressed states, secondary expiry boundaries,
and NaN/infinite elapsed values. Gate selection executes without hooks;
only transition helpers `82D82C78/82D82D78` are hooked to observe which
branch calls them. Their computations and the full reset remain outside
this oracle. The adapter is not connected to host ragdoll state or audio.

The extra-input producer is now located in physical publication
`82D8C4C8..82D8C518` (the same base-disc executable identified above).
When object `physicalPlayer +754` byte `+1A` is nonzero, it copies that
object's float `+50` to `PhysOut_State +30`. Motion publication table `+8`
byte `+1C1` selects State `+2C = 1`; otherwise byte `+1C2` selects 2,
and otherwise the selector is 0. A disabled source leaves both State fields
unchanged. State initializer `82DB9040` initializes the selector to -1 and
the value to zero (`82DB9110..114`). Source-object float and flag producers
still need tracing; these fields cannot yet be assigned host meanings.

Inactive worker `82D83F04..82D83FA0` first increments a nonnegative
`+7F8` counter; exactly 3 clears `+824/+828` and sets the counter to -1.
It then reads State `+2C/+30`: selector 0 copies the value into both extra
inputs, selector 1 copies only into `+824` and zeros `+828`, and selector 2
copies only into `+828` and zeros `+824`. Each accepted selector resets
`+7F8` to zero; other unsigned selectors retain the post-expiry state.
Consequently an accepted publication refreshes the retention interval on
every inactive update, rather than starting a wall-clock timer once.
A local oracle executes the publication and retention blocks without hooks
for 500 fixtures, checking source gating, selector precedence, exact value
bits, retained values, expiry, and counter wrap. It supplies register `r25=1`
at the publication boundary; this partial-block check does not independently
validate that register's earlier producer or the source object's semantics.
Subsequent inspection identifies `li r25,1` at `82D8C3B4` in the same
publication function, with no later assignment before the selector write.

Physical-player constructor `82D873A4..82D87404` allocates the `+754`
object with size `D4` and label `82080340`, decoded as `Physics::Wipeout`.
Its inputs at offsets 0, 4, 8, C and 10 come respectively from physical
player members `+720/+758/+75C/+718/+70C`. Source writer
`82D65DE0`, called at `82D6577C`, selects a vector from its `+10` object
at `+190` or `+260` according to the call's low-byte argument. The ordinary
branch forms the three-component dot product against virtual output `+40`
and flips its sign bit (`82D65F00..82D65F5C`). This is the float retained
in `f30` and eventually published at wipeout `+50`; it is not the solver
impulse. Its operand vectors and units still need verified producer traces.

The branch compares that value strictly against `f29` at `82D65FF4`.
The threshold was formed at `82D65EA8..82D65EB8` from authored offset
`+F8` or `+FC`, multiplied by the selected lookup scalar in `f31`.
A passing comparison sets wipeout byte `+1A=1`, stores the projection at
`+50`, and increments `+C8`; a failing comparison retains all three fields.
A local native oracle checks this suffix without hooks for 1,000 finite
threshold fixtures, including equality and counter wrap. It deliberately
supplies `f30/f29`, so it verifies the write boundary, not the preceding
VMX calculation, lookup binding, gates or operand semantics. The alternate
`+25B5` branch writes byte `+1C`, not this `+1A/+50` pair. Full source
reset and per-update clear ownership remain unverified at this stage.
The source's lookup constructs field hash `D978550D6DB4E6F7` at
`82D65E28..82D65E3C`, then calls `82B49C60` on the collection held by
its `+10` object's `+9F4` binding. The owned collections identify this field
in `physics_mode/default` as float bits `3F800000` (1.0), and in
`physics_mode/easy` as `3FC00000` (1.5). This is authored evidence for the
lookup scalar, not proof of which mode collection is bound at runtime.
The fallback address `83073F70` remains separate and unverified. Preserve
this difficulty-dependent binding rather than hardcoding the default scale.

A concrete source clear is subsequently located at `82D8B300..82D8B334`,
in physical-player method `82D8B2A8`. It zeros exactly 34 bytes beginning
at wipeout object `+14`, 34 float words beginning at `+38`, and counter
`+C8`. Thus `+1A/+50` are cleared together, while `+36/+37`, `+C0/+C4`,
and `+CC/+D0` are preserved. A local native oracle executes the loop without
hooks for 100 randomized `D4`-byte objects and verifies the entire object,
including untouched neighboring fields. This method's pointer is at physical
player vtable `82322B88 +24` (`82322BAC`); the other reference at
`82361020` is unwind/function metadata, not evidence of a second dispatch.
The method first queries physical-player member `+704` virtual slot `+1C`
and calls `82D5FF10` on member `+760`; a changed result invokes state-change
helper `82D8D840` before the unconditional clear. Its outer dispatch cadence
and ordering relative to event production and final publication still need
tracing, so this evidence does not yet justify clearing host inputs every
render or physics frame.

The 25-entry refresh has 23 ordinary values from publication bone indices
1 through 23, followed by the specific value from bone 1 and the specific
value from bone 23. Physical root bone 0 is excluded from these ordinary
entries. This reconciles the publisher's 24-bone array with the worker's
25 scoring entries and the host's separate specific-contact fields.

Filtered publisher `82DBAFF8` writes conditioner `+94` to
`PhysOut_Filtered +0` at `82DBB248`, and its prior value to `+4`.
The selected physical State `+10 == 300` branch at
`82DBB3A4..82DBB3C0` sets category 4 (Wipeout). Category 2 is Air and
7 is OffboardAir, consistent with core `physics::filtered_state`.
Native execution from `82DBAFF8` through `82DBB268` matched the existing
Rust category selector, previous-state output, five counters and change
flag for 1,000 explicit input/state fixtures, without hooks. This comparison
includes all selector branches and grind entry, but excludes grind metadata
and the later publication fields. The standalone Rust harness supplies
dummy animation name encoding, which is irrelevant to the compared fields.

The comparison exposed a counter edge outside its validated domain:
the Air-delay predicate uses the untruncated 64-bit increment result in
`82DBB18C..82DBB1A4`, while the stored counter is a 32-bit word. Core
currently compares its wrapped signed `i32` result. They disagree for an
initial air count `FFFFFFFF` (native retains a wide increment while storing
zero), and signed-overflow boundaries also require validation. The passing
comparison therefore restricts initial air counts to nonnegative values
through `7FFFFFFE`; the other counters include signed/wrapping fixtures.
This is a known unresolved boundary, not evidence of complete filtered-state
parity. Do not change the TU3-labelled implementation from this base-disc
finding until its version mapping is independently established. For the
audio worker, category 4's direct state-300 producer is established; its
State `+44/+45` suppression-byte producers remain unresolved.

Two suppression writer paths are now traced with verified pointer ownership.
Driver publication `82D13F58` receives the physical output table in `r4`.
Its block `82D140A8..82D140CC` sets State `+44` to 1 if its own signed
word `+230` is nonnegative, and State `+45` to 1 if its own byte `+1E4`
is nonzero. Failing either condition leaves the existing State byte intact;
this block is not a boolean overwrite. The source driver-field producers
remain unresolved. Another publication path `82D8C404..82D8C428` reads
owner `+760 -> +39` and, when nonzero, sets State `+45` to 1. Its
source object and writer are not yet identified.

Material helper `82D8D3C8` accepts a signed material number and the
publication table at owner `+708`. Material 6 sets State `+45` to 1
only if its prior byte is zero; materials 9 and 12 similarly set State
`+41`. Other material values leave both bytes intact. Call sites include
reading Collision `+10` at `82D8D638` and extracting tag bits 7..11 at
`82D8D798`/`82D8D7EC`. Wipeout-state branch `82D8D4CC` separately sets
State `+45` when Collision byte `+D6` is nonzero. Native execution of
the driver block and complete material helper matched 207 explicit signed,
nonzero-byte and retained-byte fixtures, with no hooks. Nonzero existing
bytes such as 255 retain their value in the material helper.

State initializer/reset `82DB9040` clears both suppression bytes. Besides
constructor initialization, the common table reset `82DBA670` calls it
at `82DBA6E0`. The common reset is itself called at construction and has
an executable address entry at `82361FA0`. That entry is in the ordered
function/unwind metadata, not evidence of a callable vtable slot. Its runtime
dispatch and cadence remain unresolved. Therefore the accumulating writer rules do not
yet establish whether the bytes last one publication or a longer interval.
Do not clear them per host frame without resolving this lifetime.

Physical publication preparation provides a more direct lifetime boundary.
Physical update `82D8C1C0` calls `82DBA848` on its table `+708` at
`82D8C1E8`, before the subsequent physical output writers. `82DBA848`
copies records from the default table rooted at `83071958`. Its
`82DBA8EC..82DBA8F8` block copies all `58` bytes of State (table member
`+1C`), including suppression bytes `+44/+45`, through original memcpy
`82F27EE0`. Native execution of this block and memcpy matched all 88 bytes
for 100 randomized source/destination fixtures, with no hooks; cache hints
have no architectural-storage effect in the local interpreter. Thus prior
live State suppression bytes are replaced at publication preparation, before
conditional writers accumulate new values. The default table's initialization
and mutability still need tracing before assuming its bytes are always zero.
Ordinary teleport also invokes this copy at `82D8DFE0`.

The activity worker's update has a direct caller at `82D7874C`, passing
owner `+30` to `82D83620`. This is distinct from the publication-preparation
owner; the enclosing scheduling and its order relative to the template copy,
physical output writers and filtered publication remain to be established.

Default-table startup initializer `82F67CF0` loads address `83071958`
at `82F67D00` and calls table constructor `82DB9D98` at `82F67D04`.
That constructor initializes its State allocation with `82DB9040` before
installing the pointer at member `+1C`; its final common reset also calls
the same State reset. Native execution of the complete `82DB9040` routine
cleared both suppression bytes in 100 randomly seeded State records without
hooks. The template therefore starts with zero suppression bytes. Startup
then registers destructor `82F710F0`, which forwards this table to
`82DBA378`. Direct-address searches identified initialization, copy reads,
and destruction; indirect mutation of template records has not been audited.

Physical update `82D8C1C0` is vtable `82322B88` slot `+3C`, whereas
the completed publication dispatcher `82D8D838` is slot `+44`.
Scoring entry `82D78580` calls the activity worker at `82D7874C`.
These verified identities distinguish physical publication preparation,
completed publication dispatch and scoring advancement; they do not prove
their outer-world order or authorize running all three on the host audio
manager's alternating phases.

Completed-publication ordering is now established inside dispatcher
`82DC7A60`. Constructor `82DC7480` installs the filtered-state conditioner
at component member `+14` (`82DC76A8`), with vtable `823239CC`
and virtual `+04 == 82DBAB08`. That conditioner calls the recovered
filtered publisher `82DBAFF8` at `82DBAC04`. The scoring component is
member `+1C` (`82DC7760`), with vtable `823239E4`, virtual
`+04 == 82DC4B60`; it calls scoring entry `82D78580` at `82DC4B78`,
which reaches activity entry `82D83620` at `82D7874C`.
Both components retain the physical table pointer passed to the constructor,
so they operate on the same State and Collision publication.

Dispatcher calls the conditioner at `82DC7B04`, then component `+18`,
then scoring at `82DC7B34`. Native execution of 25 complete dispatcher
calls confirmed all eleven component calls in order, the correct owner and
unchanged context argument, with conditioner and scoring reached once each.
Component bodies are hooked in this scheduling verifier; their computation
is not covered by this check. These identities establish that the activity
counter advances on its gated scoring path once per completed publication
dispatch, after filtered-state publication. They do not establish a fixed
wall-clock interval, the outer dispatch frequency, or whether a rendered
host frame corresponds to one completed physical publication.

Native execution of this block and core `audio::hom::ActivityCounter`
matched 1,000 explicit pair/counter fixtures,
including signed boundaries, no transitions, simultaneous transitions and
counter `FFFFFFFF`, with no hooks. This confirms the block's arithmetic and
published values, not the complete worker. The earlier update contains VMX
calculations and classification refresh calls `82D838D8`/`82D83AD8` which
are not included in this verification. Reset `82D821D8`, called from the
worker constructor at `82D821CC`, clears offset `+7F4` at `82D8222C`.
Its loop at `82D823A8..82D823CC` initializes all 25 current and previous
classifications to -1 and both accumulation floats to zero, and clears
their vectors and auxiliary fields. Runtime reset callers and update cadence
still require tracing. Other stores to that numeric offset cannot be assumed to
belong to this counter without checking their owners. The core helper takes
the current/previous 25-entry bins explicitly and returns the count, activity
float and enable byte; it does not infer their producers or update cadence.
No host state adapter
or audible attenuation was added. All addresses use the same base-disc hash.

Envelope update `82928AF8` dispatches this record to `829299F0`. The latter
has attack (state 1), sustain (3), release (4), and idle (0) behavior. Record
flag bit 8 makes sustain depend on the trigger; bit 10 permits retriggering
from release. This record has bit 8 set and bit 10 clear. Release therefore
continues even if the trigger returns during it. Attack/release transitions
preserve the current level and remap elapsed time; they do not simply start
a new fade from zero. The idle fast path at `82928B54..82928BA4` clears the
elapsed time, starting level, linear level, and log contribution when both
state and trigger are zero; records with bit 9 set instead receive -10000
as their idle log contribution. Record 76 has bit 9 clear.

For this record, the encoded attack/release curve selectors are 1/9 and the
low-12-bit attack/hold/release durations are 14/10/5. Binding at
`82927C78..82927D28` converts these with binary32 multiplication by the
constant at `822F3BA4` (16.666669845581055). These are evaluator clock values;
conversion to host seconds requires the update caller's clock contract.
Curve helper `82923EA8` is a floating-point helper, distinct from the integer
curve helper already modeled. The post-envelope multiplier path at
`8292916C..829291CC` starts at 32767, multiplies each resolved linear control
with arithmetic shift by 15, then applies the product to the log contribution.
Keep these stages and their rounding separate.

This resolves the immediate HOM producer and several reset/transition rules.
Remaining dependencies include its upstream state writers, declaration 121,
the other shared sum `30030000`, floating envelope curves, and the evaluator
clock. Runtime impact gain remains provisional; no audible change is claimed.

## Envelope float curves, update cadence, and master multiplier

Continuing against the same base-disc executable, `82923EA8..82923F14`
turns out to wrap the already recovered integer helper `82923F18`. It is not
an independent smooth curve implementation. For unsigned selectors 0..9,
it multiplies the input by binary32 32767 (`8217291C`), rounds that product
to binary32, truncates to an integer, evaluates the integer curve, then
multiplies its result by binary32 reciprocal bits `38000100` (`822F3538`).
Selectors above 9 return zero without reading a table or normalizing the input.

`evaluate_float_curve` in `tools/owned_game/mixmap_curve.py` now reproduces
this wrapper for explicitly supplied finite normalized binary32 inputs.
Out-of-domain inputs for supported selectors are rejected rather than assigned
guessed original behavior. The CLI accepts either `--input` or `--float-input`:

```sh
python3 -m tools.audio.check_mixmap_curve \
  --table .local/audio-investigation/mixmap-curve-table.bin \
  --float-input 0.5 --curve 1
```

With the owned table this returns `0.7059236168861389`. Three added tests
check quantization, a multiply-rounding boundary, domain rejection, and the
unsupported-selector bypass. In particular, input bits `3F000100` multiplied
by 32767 lie just below 16384 in binary64, but native `FMULS` rounds to 16384
before integer conversion. Omitting that rounding changes the curve input.
The focused suite passes 74 tests. Reuse/quality/efficiency review found only
a stale module description, which was corrected. This extends the offline
reference; it does not yet run the envelope state machine in gameplay.

The clock path is also narrower now. Evaluator `82927E90` stores its incoming
float at +84, multiplies it by 1000 (`82251F78`) for envelope delta +88, and
separately by binary32 29.970001220703125 (`822F3BA0`) for field +22C.
Wrapper `82924398` forwards the float to the evaluator when enabled. Both
manager call sites (`8247328C`, `824734D0`) pass accumulated register f30.
Manager entry `82473060` forms f31 from saved +1DC plus incoming f1 and f30
from saved +1E0 plus incoming f1, using binary32 additions. With incoming
f1 at most the binary64 constant 0.02 (`822F3A88`) and force byte r5 clear,
it alternates family input publication and MixMap/output processing using
word +1E4's low bit. Each phase clears its own saved delta and retains the
current input delta for the other phase. Larger deltas or a forced update
run both phases and clear both accumulators. Its disabled branch and upstream
caller's time source still need tracing. The evaluator's multiplication by
1000 is consistent with milliseconds, but the root time source and pause/time
scaling contract are not yet established. Preserve this batching when adapting
the verified clock; render-frame cadence alone is not evidence of parity.

The envelope multiplier's `48000020` controller is object `SFXObj_Master`,
registration `82FD13D0`, factory `824C2AD0`, vtable `822F6A98`, update
`824C2CD0`. Input slot 0 is 32767 exactly when global context `*(83027DA0)`
is present and helper `8277ADC0` returns a nonzero low byte; otherwise it is
zero. That helper gets an object via context +64's virtual +10, returns null
if absent, otherwise gets it again and calls that object's virtual +10.
The concrete implementation behind those virtual calls remains unresolved.
The declaration's curve selector 8 complements the input as `32767 - input`;
this is a state-dependent control, not the user's master-volume slider.
Slots 1/2/3 separately read floats at settings-object +88/+80/+84, multiply
by 32767, truncate and clamp. Do not conflate these publications with slot 0.
Full declaration processing and the concrete state query remain prerequisites
for replacing the live multiplier.

### Loud ragdoll playback: Master settings recovery

The reported loud bails expose the current full-scale controller adapter in
`skating_audio/collisions.rs`: every packed volume slot receives 32767.
The original selector can enqueue primary body, surface, cloth and bone
layers together, so this bypass affects more than one simultaneous voice.
This is a likely contributor, not a measured original-game loudness comparison.
The user chose recovery of original mix controls instead of a temporary host
volume reduction. No ragdoll gain multiplier has been introduced. The
recovered controls are now live; see
[Live collision volume controls](#live-collision-volume-controls).

`audio::master::SettingsInputs` now ports base-disc
`824C2D48..824C2E58` using the executable hash recorded above. The settings
pointer is `*(8300B248 +1C)` (`824C2D50..824C2D58`). Settings floats
`+88/+80/+84` are multiplied in single precision by 32767 (`8217291C`),
truncated to signed words and retained at Master `+1C/+20/+24` **before**
clamping to 0..32767 for controller slots 1/2/3. Those raw retained values
must not be replaced by the clamped controls. A missing bound controller
suppresses writes without suppressing the raw conversions/stores.
Owned Main declarations 4/46/49 read slot1, 5/51 slot2 and 3/47/50 slot3;
their authored references remain separate from Master slot0's state query.

Local `verify_master_settings.py` executes that original instruction region
with only controller writes hooked. Production Rust matches all 2,000
publications, raw object values and controller words, including fractional,
negative and above-one settings. The port reuses the verified checked integer
conversion and rejects the unsupported PPC invalid-conversion domain.

Further tracing identifies context `+64` as a constructor argument retained
at `8277987C`: caller `826B87A0..826B87FC` passes the object found through
`*(83027D34) +98`. This narrows the remaining slot0 concrete virtual-dispatch
investigation; it does not justify mapping that query to a host bail flag.

This settings stage is not connected to live playback yet. The settings
object's writers/lifetime, concrete slot0 query, remaining envelope/modulation
input producers and full graph/control-owner update still require recovery.
Full-scale collision controls remained in runtime at this point, so the
reported ragdoll loudness was **not fixed** by this isolated input port. It is
now fixed by [Live collision volume controls](#live-collision-volume-controls). The authored slot18
write's -1100 adjustment is not a stand-alone gain: its final lookup also
consumes evaluated output/modulation levels. The explicit zero-level reference
writes 9224, but those zero inputs are not recovered live gameplay defaults.

## Gated collision envelope state reference

`tools/owned_game/mixmap_envelope.py` now implements the linear state machine
at base-disc `829299F0..82929CE0` for type-1 envelopes with flag bit 8 set
and bit 10 clear, the combination used by `B100304C`. This continues against
the same executable hash recorded above. Attack/release durations, selectors,
the owned curve table, the trigger, and evaluator delta are explicit inputs.
This is an offline reference, not a host frame-time or bail-state adapter.
It excludes the surrounding logarithmic conversion, control multipliers,
other flag combinations, and controller ownership/lifetime.

Recovered and implemented behavior:

- Idle with no trigger clears state without advancing time, matching the
  outer evaluator's fast path (`82928B54..82928B84`).
- Attack advances in binary32 clock arithmetic. Losing the trigger is handled
  before attack completion, captures the previous output level, and remaps
  elapsed time into release. If the update overshoots attack duration, this
  offset can be negative; clamping it would change the original sequence.
- Attack completion discards overshoot and enters sustain. Sustain publishes
  32767 and clears elapsed time on each active-trigger invocation. Its release
  transition also discards that invocation's elapsed time.
- Release uses the captured start level, the recovered float curve, separate
  binary32 subtraction/multiplication, and truncation. A renewed trigger does
  not interrupt release. Release completion resets to idle even if the trigger
  is already active; attack starts on a later invocation.
- The attack-abort duration branch compares against original constant bits
  `418553F8` at `822F377C` (16.666000366210938), distinct from the duration
  decoding scale bits `41855557` at `822F3BA4`. Preserve both values.

Seven synthetic tests cover the full attack/sustain/release sequence,
interrupted attack, negative remapped offsets, ignored retriggers, short
attack completion, resets, input rejection, and atomic rejection of remapping
overflow. The reference deliberately rejects nonfinite/overflowing diagnostic
inputs rather than inventing original behavior for those cases.

An ignored local instruction-subset interpreter executes the owned
`829299F0` routine and its integer curve helper, comparing phase, elapsed
time, remapped offset, captured level, and output level after each invocation.
All 12,000 deterministic sequence comparisons passed across three duration
pairs and four selector pairs, including the owned HOM durations/selectors,
zero and large deltas, trigger changes, and unsupported curve selectors.
The local harness supplies the separately traced outer idle gate and delta
increment; this is not an end-to-end original-game recording. The complete
focused suite passes 81 tests. Simplify review caught the overflow-rejection
issue; it is fixed and both validation runs pass afterward.

The audio manager's disabled branch is now checked too: `82473074..82473080`
jumps directly to the return at `824732D8`. It neither advances nor clears the
saved clock accumulators or alternating phase on that invocation. This does
not prove that other lifecycle methods never reset them. The manager update
appears in its vtable at `822F69D4`; the root caller and concrete master-state
virtual query remain unresolved.

No runtime playback behavior changes in this step. The newly complete linear
envelope slice is ready to compose with its remaining original dependencies;
it must not be triggered directly from host contact force or a guessed bail
flag. The reported missing ragdoll impacts remain open.

The Rust counterpart is now `skate_core::audio::envelope::GatedEnvelope`.
It ports the same bounded flag combination and linear fields against base-disc
`default.xex` SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`,
addresses `829299F0..82929CE0` and the outer idle gate
`82928B54..82928B84`. It takes explicit evaluator milliseconds, resolved
trigger and owned curve table; it does not infer any of them from host state.
The local owned-instruction harness compared all five runtime state fields
bit-for-bit across 12,000 sequential updates, including the authored HOM
durations/selectors, interrupted attacks, negative offsets, release retriggers
and unsupported selectors. All comparisons passed. The Rust audio suite passes
72 tests; three simplify reviewers found no worthwhile changes.

This remains an isolated linear stage. Log conversion, authored multipliers,
other envelope flags, spatial modulation, live Master settings and state-input
producers still need composition and validation before replacing the gameplay
collision controller's provisional full-scale volume words. (These are now
composed; see [Live collision volume controls](#live-collision-volume-controls).)
No temporary volume reduction was applied.

### Collision envelope flags and logarithmic contribution

A fresh traversal of the owned collision output dependency cone confirms
15 declarations, 18 envelopes, one sum and two modulation nodes. The envelope
records are Main indices 1, 4, 5, 10, 11, 15, 25, 48, 58, 69, 76, 79, 139,
178, 219 and 228, and Player indices 7 and 20. All are type 1 with bit 8 set
and bit 9 clear. Main index 1 at file `06A8` also sets bit 10; the other
17 clear it. Raw zero attack ticks occur at Main indices 139, 178 and 228,
and Player indices 7 and 20. This uses the same executable/file hashes above.

Constructor `8292B3A0..8292B3E4`, together with its shared branch
`8292B2B8..8292B2DC`, replaces zero attack/hold/release ticks with one before
binding scales durations. Rust `GatedEnvelope::from_authored` now preserves
that attack/release normalization and the four-bit curve selectors.
The bit-10 release branch at `82929A84..82929AF4` checks completion before
retriggering; completion still resets to idle. Otherwise a renewed trigger
captures the previous linear level and remaps remaining release time into
attack, using the same original short-duration comparison. The Rust helper
now supports this second gated flag combination. Forty thousand sequential
state comparisons against owned instructions passed, covering both bit-10
values, zero authored ticks, maximum ticks and unsupported curve selectors.

`audio::envelope::LevelControl` ports the bit-9-clear logarithmic stage at
`82928C08..829291CC`. It reuses verified authored magnitude-to-depth table
conversion. Positive authored levels compute shaped linear value as
`32767 - depth + (linear * depth >> 15)`, then add the positive base to its
log-table result. Nonpositive levels instead shape as
`32767 - (linear * depth >> 15)`. Rewriting either expression into the scalar
declaration's subtraction-first form changes rounding. A null multiplier
array skips weighting; a present empty array still weights by 32767. Present
arrays execute their low-byte count, multiplying resolved linear words in
binding order with separate arithmetic shifts before weighting the log result.

Five thousand additional cases matched the owned outer envelope evaluator's
log and multiplier instructions. The harness supplies metadata from the
separately verified table conversion and hooks only the state helper to retain
an explicit linear input; it does not execute original producers or constitute
a live game trace. Tests cover sign-dependent rounding, null versus empty
multiplier arrays and zero/invalid controls. Simplify review consolidated the
ordered multiplier product shared with scalar declarations while preserving
their different null-array behavior. The Rust audio suite passes 74 tests.

These ports cover the collision cone's envelope flag combinations and its log
arithmetic. Authored instance binding, source publication, ownership/lifetime,
spatial modulation and playback composition still remain; no audible loudness
fix or complete sound-mix parity is claimed. (The volume composition is now
live; see [Live collision volume controls](#live-collision-volume-controls).)

### Authored collision envelope binding and complete node replay

`audio::envelope::mxb::load` now loads selected dependency records from each
family's +18 section and binds all explicitly configured instances. Original
constructor `8292B148..8292B678` traverses `6 + ((word1 >> 16) & 15)` words
per record; selected records whose binder low-five-bit count disagrees with
that stored count are rejected. Type/flag combinations outside gated type 1,
bit 9 clear remain unsupported. Unselected records are traversed without
executing or substituting their behavior.

Source construction `8292B610..8292B624` ORs the instance into the authored
source, preserving existing instance bits. Multiplier binder
`829267B0..829268AC` compares each reference family against the **envelope
descriptor's family**, not the source family used by scalar gates. Matching
families OR in the containing instance; other families expand over their
explicit counts in ascending order, also retaining authored instance bits.
An authored empty list produces a null pointer. A nonempty list expanded over
zero instances remains a present empty array. The bound representation keeps
this difference through evaluation.

`BoundEnvelope::advance` composes the state machine and logarithmic controls
with explicit source words and resolved linear multipliers. A full nonzero
source word triggers; it is not narrowed to a 15-bit amplitude. The outer
idle-with-zero-source branch skips shaping and weighting, publishing zero
for this supported bit-9-clear combination. Completion inside the helper
still proceeds through shaping, as in the native outer loop. Source and
multiplier producers, evaluator milliseconds and owned tables remain caller
inputs. No host contact/bail shortcut supplies these controls.

Resolver `82927744..82927758` follows a scalar declaration to its shared curve
linear +0C field when r5=0. `ScalarProgram::linear` now exposes that value
separately from its logarithmic declaration level for envelope binding.

A local replay executes the actual owned constructor and multiplier binder
for all configured Main/Player/Collision instances, then compares all source
and multiplier IDs for the 20 collision-relevant envelope instances (16 Main
and two Player records instantiated twice). All match Rust. The native outer
evaluator, state helper, float/integer curve helpers, logarithmic conversion
and multipliers then execute without behavioral hooks. Across 300 sequential
frames, all 6,000 selected node updates matched phase, elapsed/offset float
bits, starting/output linear levels and log levels. Resolved input words are
explicit shared synthetic values; this is complete selected-node replay,
not full-graph or original-game live-state validation. Three loader tests
cover descriptor/source family differences, retained instance bits, missing
and malformed selections, unsupported flags, null/empty arrays, idle bypass
and atomic diagnostic rejection.

### Master volume profile source and settings menu

The Master settings pointer's producer is now identified. Global service
initialization `824E94F8` constructs a 0x290-byte object with `824E8888` at
`824E9994`, storing it into `8300B248 +1C` at `824E9998`. The constructor's
name key at `822F819C` is `SKATER_PROFILE`. At `824E8950..824E8964` it calls
`824ED4D8` on profile +80. That helper writes original float 1.0 (`82314D90`)
to +00/+04/+08, establishing **profile** +80/+84/+88 construction defaults.
This is an original lifecycle default, not a neutral-reference MixMap value
or host playback multiplier. Base-disc executable/hash are unchanged.

Menu label lookup `8258EC88..8258ECB4` indexes eight-byte entries from table
`82FCB168` using the same selected index consumed at `825F360C`. Entries 7/8/9
are `ID_GAMESETTINGS_SFXVOLUME`, `ID_GAMESETTINGS_DIALOGVOLUME` and
`ID_GAMESETTINGS_MUSICVOLUME`, at `82FCB1A0/1A8/1B0`. Settings event routine
`825F3F48..825F418C` selects the live profile +1C pointer and edits profile
+80/+84/+88 respectively. Original event 14 uses -1.0; event 15 uses +1.0.
Each update executes FMADDS with step `820641A0` (bits `3DCCCCCD`), clamps to
0..1, then snaps values **strictly below** `82163B20` (bits `3D4CCCCD`) to zero.
Rust `audio::master::ProfileVolumes` now preserves the construction defaults,
field identities and this field-changing menu slice, using fused `mul_add`.
Its publication keeps Master slot order music/effects/dialogue (1/2/3).

Profile reader `824ED730` conditionally deserializes +80/+84/+88 through
virtual +44 in that order; writer `824EDEB8` serializes them through virtual
+1C. The menu snapshot `825F3130` copies these fields into +58/+5C/+60 of the
menu object. This establishes persistence and snapshot routes, but the complete
reader format, every profile writer/reset and lifetime transition have not
yet been ported. UI notifications and save scheduling remain outside the
volume-field helper. The macOS global playback gain stays a separate host
setting, without an inferred correspondence to these profile fields.

The original defaults and 5,000 finite profile-setting menu updates matched
production Rust bit-for-bit against owned constructor/menu instructions,
including clamp/snap neighbors and profiles outside 0..1. The same replay then
executes the Master publication region and compares all three raw converted
values and clamped controls. Only the unrelated menu-click function is hooked;
the float arithmetic and Master conversions execute natively in the local
instruction interpreter. Three simplify reviewers found no worthwhile changes.
The Rust audio suite passes 78 tests. Ragdoll playback still needs live state
producers, spatial modulation and complete graph/control-owner integration.

## Shared sum evaluation and corrected instance binding

After commit `e30b4e2`, tracing the binding stage exposed the correction above:
authored reference bits 11..15 are not necessarily the final runtime instance.
Base-disc `8292B928..8292BA90` binds +0C shared sums. Its `RLWINM` mask
`FFFF07FF` clears those bits before assigning instances. A reference to the
containing family receives that containing instance. A reference to another
family expands into one reference per configured instance of that family,
in ascending order. Families with count zero contribute no references.
Duplicates remain duplicates and authored argument order is preserved.
Instance counts come from evaluator fields indexed by `4 * (family + 2)`;
they must not be assumed from the source identifier.

`8292BAA0..8292BCBC` separates output-record modulation references from sum
references. Modulation references receive the output's containing instance;
its non-modulation arguments use the same cross-family expansion rule at
`8292BC20`. Collision output record `4534` therefore binds its modulation
`90030000` and shared sum `30030000` to the containing collision instance,
while expanding `B100304C` across the configured family-0 instances.
The envelope producer traced earlier is still applicable to each such
instance; the earlier claim of a fixed instance 6 was premature.

The collision shared sum's 16 authored arguments at `44DC` contain fourteen
family-0 references and two family-1 references. Its resolved count is thus
`14 * count[0] + 2 * count[1]`, not necessarily 16. This says nothing about the
actual live counts, which remain to be established.

`tools/owned_game/mixmap_sum.py` now models this binding rule and the resolved
sum stage at `829283DC..82928458` using explicitly supplied counts/levels.
It reproduces the following details:

- A null runtime reference-array pointer retains the previous accumulator.
  A present array resets the accumulator, including when its count is zero.
- Evaluation uses only the low byte of the expanded reference count. Counts
  256 and 257 therefore execute zero and one entries respectively. Do not
  replace this with summing all expanded entries.
- Each addition stores a 32-bit word; signed overflow wraps before comparisons.
  Clamping occurs after accumulation, first to the upper bound then the lower.
- The upper bound is `(clamp_word >> 16) & 0x7FFF`. The lower bound is produced
  by OR with `FFFF0000`, equivalently `(clamp_word & 0xFFFF) - 65536` as signed
  arithmetic. It is not ordinary signed-halfword extension. For the collision
  record's `0000D8F0`, the final range is -10000 through 0.

Six synthetic tests cover binding order, missing/zero instance counts,
instance-bit replacement, overflow, clamp decoding, null versus empty arrays,
and count truncation. An ignored local instruction-subset interpreter executed
the owned sum routine for 1,000 deterministic cases, including signed overflow,
null arrays, and counts around 255/256; all matched the reference helper.
Simplify review found no worthwhile changes. This does not yet resolve the
values of those referenced controls or change runtime playback.

## Native scalar declaration evaluation

`audio::scalar` now constructs and evaluates scalar declarations using the
owned log and volume tables supplied as big-endian buffers. The constructor
at 8292A89C..8292AD3C converts the magnitude of the signed authored halfword
through the volume table, stores depth `32767 - converted`, and retains a
positive authored value as the additive base (otherwise zero). It preserves
the shift cutoff, including the signed minimum input.

Evaluation at 82928140..829283A0 computes:

```text
shaped = 32767 - (((32767 - source_linear) * depth) >> 15)
level_before_gates = base + original_log_table_conversion(shaped)
weight = 32767
for each resolved linear gate: weight = (weight * gate) >> 15
level = (weight * level_before_gates) >> 15
```

Multiplications retain native word wrapping and arithmetic shifts. The log
conversion preserves sparse-bin upper-end selection. Gates are linear values,
not logarithmic declaration levels. Binding helper 829266B8 expands the authored
references, then overwrites descriptor byte4 with the low byte of the expanded
count at 82926790..82926798. The evaluator reads this rewritten byte. The raw
file's five-bit argument count is therefore not the runtime iteration count.
Null gate arrays leave the initial 32767 weight, and count 256 evaluates no gates.

The same native module now supplies ordinary enabled format-0 packed volume
conversion, sharing the attenuation table helper with declaration construction.
A synthetic integration test runs declaration -> output accumulation -> packed
volume -> collision voice gain, preserving intermediate integers. This is an
explicit-input chain, not yet a live controller owner in gameplay.

Five new Rust tests cover constructor signs and boundaries, logarithmic bins,
fixed-point gates, expanded-count wrapping, invalid tables and the composed
voice path. A local interpreter of the owned declaration evaluation loop
matched compiled Rust for 2,000 cases, including null gates and counts
0/1/3/255/256/257, comparing shaped linear, log, gate weight and final level.
Constructor metadata for that comparison was independently derived from the
owned volume table and traced initialization rules; the run does not emulate
allocation or reference binding. Simplify review found no further changes.

Remaining work is loading/binding the program's actual curve sources and
controllers, advancing them with verified publication timing, and connecting
scheduled collision records to playback. Passing this arithmetic chain does
not establish audible parity or resolve the silent-ragdoll report.

## Native curve stage and linear-reference binding

`audio::curve` now implements the integer curve helper 82923F18..82924100 and
float wrapper 82923EA8 using the caller-supplied 513-word owned table. It retains
the unusual interpolation mask, endpoint guards, complement/squared variants,
and float rounding. Invalid selectors return zero before validating the source,
as the evaluator/wrapper does. Valid inputs outside the verified domain fail
instead of being clamped.

Shared curve nodes initialize their log field to 0 and linear field to 32767
(829266A8..829266B0). Evaluation updates both fields through the recovered curve
and log conversion. The default pair must not be replaced by the evaluated pair
for source 32767: those need not be identical.

The reference resolver distinction is now explicit. At 82927748, the scalar
reference resolves to declaration runtime state. For r5=0 it follows that
runtime state's +00 pointer to the shared curve node, then returns node+0C
(linear); for r5=1 it returns declaration runtime+08 (evaluated level).
It does not return the declaration metadata's similarly named +0C depth field.
Curve-source and gate binding pass r5=0 at 82927B6C/82927BBC; output-reference
binding passes r5=1 at 82927D94. Live execution must preserve these distinct
objects and the original shared-curve update order.

Four new tests cover selector behavior, native interpolation weights, float
wrapper validation and a curve -> declaration -> output chain. Production Rust
matched the existing owned-instruction interpreter on 37,439 cases, including
all 32,768 inputs for the body curve. All 29 filtered core audio tests pass.
Simplify review consolidated the duplicated integer-table reader; scalar
instruction comparisons were repeated successfully after that refactor.

This extends the executable arithmetic chain, not the runtime voice scheduler.
Owned program loading, reference binding, live controller production and update
ordering still need integration before these values can replace provisional
playback gains and pitches.

## Bound scalar program runner

`audio::scalar_program` now loads the original MixMap scalar declaration section
and binds its shared nodes and gates. `from_mxb` accepts explicit factory counts,
checks big-endian offsets/strides and supported declaration limits, and treats
directory FFFFFFFF as an absent family with effective count zero
(829256D8..82925700). It does not infer counts from current player activity.

Construction preserves the distinctions recovered from 829265C8, 829266B8 and
8292A6A8: source IDs clear authored instance bits before assigning the containing
instance; gate references OR instance bits without clearing them; external gate
families expand over configured instances; source nodes share state only when
their full constructed IDs match. Curve nodes execute in selector-bin order,
preserving first occurrence within each bin, then declarations evaluate in
construction order. Cross-node reads see the original in-place update order,
including values retained from the preceding phase. There is no topological
reordering or guessed reset on player state changes.

Non-scalar input references remain explicit required linear values from the
other original producers. The runner exposes canonical owner/slot keys for
unpacked word inputs; other reference kinds retain their reference words.
Missing, short or out-of-domain input arrays fail before changing program state.
This partial runner does not supply envelope/modulation/actor inputs, run shared
sums or output writes, or schedule voices. Repeated phases allocate no memory.

The owned `.mxb` was loaded with the verified Main 1, Player 2 and Collision 10
instance counts (other families omitted from this scalar replay). This produced
444 declarations, 181 shared curve nodes and 104 explicit external references
after the word-input alias correction described below (previously 146).
A separate reference evaluation matched every declaration level across 100
sequential synthetic-input frames. This checks program parsing, construction,
binding and state order composed with the recovered arithmetic; it is not a
capture of actual controller buffers or a whole-program PPC emulation.

Six new tests cover state order across phases, source-node sharing, gate
expansion, missing scalar references, binary stride/sign handling, malformed
input and the absent-family sentinel. Simplify review found and fixed the
sentinel/count discrepancy and found no other changes needed. Host playback
still needs the original producers and scheduler connected to this runner.

## Controller word binding and Master publication-speed inputs

`audio::input` implements unpacked controller word ownership from base-disc
8292797C..82927AC0, using the same default.xex hash recorded above. Reference
kinds 4 and 6 select a 16-word buffer using owner mask E0FFFFF0 and the low
four bits as the slot. Curve flags do not create separate input words. The
scalar runner now deduplicates these aliases while retaining distinct curve
nodes. Explicit attachment initializes a buffer once; subsequent attachments
preserve its values, and missing owners remain distinguishable from zero.
These buffers are separate from packed voice output controls.

`audio::master::PublicationSpeed` ports Master slots 4, 5 and 11 from
824C2E58..824C3010. It consumes the original publication header, defaults to one
only for absent context, and applies the aud_general/treatments setting
Hash_A12258EB71B6937A only when context exists, manager byte 34B is set and the
original header is below one. Low/high speed controls preserve float32
arithmetic, truncation and the original bounds; slot 11 reflects the resulting
speed. Writes preserve all other Master slots. The caller must supply the
resolved setting and original header; host frame delta is not a substitute.

The speed arithmetic matched 2,009 cases against the owned instruction block
824C2EE8..824C2F54. Rust tests separately cover treatment conditions and retained
words. A composed test publishes Master controls into the word buffer and
evaluates two differently curved aliases through the scalar program. The owned
444-declaration replay still matches all 100 sequential synthetic-input frames
after canonicalization. All 39 tests selected by the core audio test filter
pass; simplify reviews found no further changes needed.

This connects a recovered producer to the native evaluator in tests. The live
game still needs publication inputs, remaining producers, output binding and
voice scheduling; this is not an audible impact fix or gameplay parity claim.

## Native shared-sum binding and composed volume path

`audio::sum` ports shared-sum reference expansion from base-disc 8292B928 and
evaluation from 829283DC..82928458, using the executable hash recorded above.
Binding clears authored instance bits, selects the containing instance for
same-family references and expands other families over explicit factory counts.
Duplicates and authored order are retained. This differs from scalar-gate
binding and is kept separate. The expansion helper also matches non-modulation
output-argument binding; connecting those output records remains outstanding.

`SharedSum` accepts expanded references, the authored clamp word and explicit
prior state. A null runtime array retains that state; a present empty array
recomputes zero. Evaluation visits only the low byte of the expanded count,
adds with signed 32-bit wrapping and then clamps. The lower bound is formed by
OR with FFFF0000, not sign extension of the stored halfword. Missing producer
values return an error without replacing the prior state. Evaluation allocates
no memory and does not invent owner lifetime or resets.

Production Rust matched 1,000 cases against the owned sum instruction block,
including null/empty arrays, expanded counts above 255 and overflowing inputs.
A composed test runs two scalar instances through reference expansion, shared
sum, output accumulation, format-0 attenuation and the body-class volume slot
into the collision gain consumer. That fixture uses synthetic tables and is a
composition check, not an original gameplay trace. All 43 tests selected by the
core audio filter pass; simplify reviews found no changes needed.

The composed test does not establish that the complete collision graph or live
impact playback is connected. Authored loading is now implemented below;
runtime producer resolution remains outstanding.

## Authored shared-sum section loading

`audio::sum::mxb::load` reads the original group +0C section. Constructor
8292AD98..8292AF20 establishes the signed relative section offset, 16-byte
section header, record stride of eight bytes plus four per authored reference,
and zero initialization of each instance's accumulator at 8292AEE8. The
authored reference count is `(descriptor >> 16) & 255`. Runtime indices follow
file order, independently of the descriptor's low byte. The loader retains
family/instance/file construction order and applies the recovered expansion
rule with explicit factory counts. Directory FFFFFFFF forces the effective
count to zero both for construction and references from other families.

Scalar and sum loaders share an aligned, bounded big-endian word reader. Sum
loading rejects truncated or misaligned records and unsupported counts, rather
than manufacturing missing records or values. A negative section offset
produces an instance with no sums. Loading prepares present reference arrays;
null-array retention remains available for explicit runtime captures through
`SharedSum`, without introducing a guessed transition to null.

The owned MixMap file with Main 1, Player 2 and Collision 10 instances produced
58 sums. All expanded references matched an independent parser/binder, and all
5,800 evaluations across 100 synthetic-input frames matched the owned sum
instruction block. This checks authored loading composed with evaluation; it
does not capture live controller levels or emulate the complete constructor.
All 45 tests selected by the core audio filter pass, including absent-family,
stride, instance and malformed-file cases. Simplify reviews found no further
changes needed. Runtime producer resolution, output binding and scheduling
still prevent claiming live impact parity.

## Collision output bindings and packed writes

`audio::output::OutputBindings` implements 8292BAA0..8292BCBC: modulation
references occupy an authored prefix and select the containing instance,
including cross-family references. Remaining level references use shared-sum
expansion. The native code counts modulation references and then consumes that
many leading entries; it does not sort them. All 262 owned output records
satisfy this prefix requirement. The port rejects unsupported interleaving.

`write_collision_controls` implements formats 0 and 1 used by all five collision
output records. The writer reads the owner-enabled low bit in word 15 and
preserves original map order and neighboring packed halfwords. It uses the low
five bits of the write count and modulation count. A map's modulation index
outside that count selects the direct path. A high-bit special write copies
the selected modulation runtime +08 low halfword; without a selected node it
uses ordinary direct conversion. Ordinary volume and pitch consume distinct
modulation fields +0C and +14 and reuse the verified arithmetic helpers.

Tracing 82928A68..82928AD8 established that a disabled record writes only its
first map entry, even when the enabled-path execution count is zero. It does
not loop over or clear the other mapped slots. Format 0 writes raw D8F0 and
format 1 writes zero. The writer preserves this behavior. Missing map data or
unsupported formats fail before modification; other formats remain unported.
Owner attachment, initialization and subsequent lifetime remain separate.

Two thousand composed writer cases matched the reference arithmetic with the
owned volume table. Of these, 667 disabled cases were compared directly with
execution of the owned 82928A68..82928AD8 instruction block. Enabled comparisons
compose separately verified conversions with reference routing/packing; they
are not full writer emulation. The scalar-to-sum-to-voice integration test now
uses the packed writer for body volume slot 18. Simplify reviews found no
further changes needed. Authored output-section loading, live producers and
voice scheduling still need connection before impact playback can use this
path or be called faithful to the original game.

## Authored output sections and buffer assignment

`audio::output::mxb::load` now reads the paired group +10 output records and +14
write maps, then applies `OutputBindings`. The loader retains the signed high
halfword base, raw mapping header and all low-byte-count stored write entries.
Execution still uses the writer's separate low-five-bit count. Directory and
effective family counts share a bounded reader with the sum loader.

Constructor 8292AF28..8292B144 assigns packed buffers by adjacent authored owner
ID: matching the preceding owner reuses its buffer; a changed owner advances
to another buffer. A nonadjacent repeat therefore receives another buffer,
not a global-ID deduplicated allocation. Runtime owner construction at
8292B0C4..8292B0D4 ORs in the containing instance without clearing existing bits,
unlike argument binding. These rules are preserved in the returned records.
Invalid storage counts, first-owner zero, malformed offsets and truncated maps
are rejected. This loader describes storage; it does not attach controllers or
infer their enabled state.

Every field of 524 instantiated records from the owned file matched an
independent parser/binder: all 262 authored records, instantiated twice for this
test, including owner IDs, buffer indices, bases, maps and argument references.
Those two-per-family counts are a test configuration, not a claim about the
live factory registry. Unit tests also cover absent families, owner-bit
preservation, adjacent and nonadjacent repeats, storage bounds and a stored
32-entry map whose enabled execution count is zero. Simplify reviews found no
further changes needed. Live producer resolution, owner attachment and voice
scheduling remain outstanding; authored loading alone does not fix playback.

## Post-producer level stage

`audio::level_program` composes loaded shared sums and output accumulators,
resolving scalar levels and local sum/output references while keeping other
producer levels explicit. Base-disc evaluator `82927E90` runs each stage over
all instances before the next: curve nodes, declarations, modulation
(`82929F00`), envelopes (`82928AF8`), sums (`829283C4..82928470`), outputs
(`82928470..82928520`), then packed writes (`82928530`). Construction at
`82924768..829247C8` appends each family's instances, and each instance's sums
before its outputs, to the global arrays (`8292AD98`, `8292AF28`). The port
preserves this order. Sums and outputs accumulate in place, so a reference to a
later node reads its previous-phase level. The owned Main family has two such
forward reads: sum 2 reads sum 9, and sum 3 reads sum 4. Sums start at zero
(`8292AEE8`) and outputs at -10000 (`8292B0B4`).

The resolver (`829276A0`) dispatches on the top three bits. Kind 0 selects a
declaration and kind 1 a sum (bit 28) or output. Kinds 2/3 select controller
words, kind 4 modulation and kind 5 envelopes. Sum and output references are
resolved with r5=1 (`82927D54`, `82927D98`): declaration runtime +08, sum +04,
output +08, envelope +18 (log) and modulation +0C. Scalar gates and envelope
sources instead use r5=0 and read linear fields. Envelope and modulation
lookups ignore reference bits 8..10 and 24..28, so external envelope inputs are
keyed by `id & E0FFF8FF`. Kinds 6/7 resolve to one static word at `830282F4`,
whose producer is untraced; the loader rejects them. The owned sums and
outputs reference only declarations, sums, envelopes and (as output prefixes)
modulation.

Five Rust sequence tests cover forward and backward reads across phases,
disabled owners publishing -10000 to later readers, separate scalar and
cross-family sum binding, resolver-identity input keys, unchanged state after
invalid frames and rejected self, missing and static-word references.

The local `verify_level_program.py` binds the owned graph independently in
Python, lays it out like the native constructors and binders, and executes the
owned `829283C4..82928520` block with an instruction-subset interpreter. Scalar
levels come from the Rust scalar program, which `verify_scalar_program.py`
checks separately. With Main 1, Player 2 and Collision 10 instances, 58 sums,
299 outputs and 232 envelope inputs matched across 200 sequential frames
(71,400 comparisons). The frames include 20% disabled buffers, overflowing
inputs, and two quiet opening frames that expose construction state. Mutation
checks confirmed that the replay detects reversed sum order and a wrong
initial sum level. A wrong initial output level is not observable in the owned
graph, because no owned output reads another output; the synthetic test covers
that case.

This validates the level stage, not its producers. Envelope, modulation and
controller-word inputs remain explicit, and packed writes, voice scheduling
and playback are not connected. The collision outputs' dependency cone contains
15 declarations, 18 type-1 envelopes, one shared sum and two modulation nodes.
Their controller words come from `SFXObj_Master`, `CameraMan`, `NIS`, `Pause`,
`Speech`, `Bloom`, `VU`, `Challenge`, `HOM` and `Menu` (Main family),
`SFXObj_Contacts` and `SFXCTL_PlayerPhysics` (Player family), and
`SFXCTL_3DColPos`. Each needs its producer traced before the body volume slot
can be computed live; those names come from the registration table at
`82FD1310..82FD15E0`.

## Live collision volume controls

The reported loud ragdoll impacts came from the full-scale controllers above:
every collision voice used 32767 as its material-class volume slot. The
original computes those slots per collision group with the MixMap program.
`skate_core::audio::collision_mix` now evaluates them live from the `mixmap/`
export, which the collision bank requires. Addresses use the base-disc
`default.xex` SHA-256 recorded above.

**Cone.** The voice gain reads slots 12..21 (`824BFF88`). Collision output
records 0 and 3 (file `4534`, `4580`) write them. Following references from
those records, as the binders do, finds 8 declarations, 18 envelope records
(16 Main, 2 Player; 20 instances), and each group's shared sum 0 and
modulation node 0. It is the volume subset of the collision outputs' full
cone described in [Post-producer level stage](#post-producer-level-stage).
Pitch records 1 and 4 (slots 1 and 22) are now built with six more Main
declarations and modulation node 0
([Collision pitch controllers](collision-pan.md#collision-pitch-controllers)). Record 2
(slots 2..11), with Collision declaration 0 and modulation node 1, is not built.
The cone is evaluated in the stage order
of `82927E90`: curves and declarations, modulation, envelopes, sums, outputs,
packed writes. Its inputs and their producers:

| Producer | Words | Free-skate value | Evidence |
| --- | --- | --- | --- |
| SFXObj_Master `824C2CD0` | 0, 2, 3, 6 | Word 0: 0. Words 2 and 3: 32767, the SFX (`+80`) and dialogue (`+84`) profile volumes at their 1.0 defaults. Word 6: 32767 on the first input phase only | Provider `82A248A0` returns 0; profile defaults `824ED4D8`. The word 6 timer `+40` is 0.0 at construction, 0.8 on entering FE state 2 and 0.5 on leaving it; it publishes while not below zero |
| SFXObj_HOM `824DC1F8` | 0, 2 | 0 | Slot 0 needs manager `+34B` and a ratio below 1; slot 2 needs `+36C` = 31 with `+1CC` set |
| SFXObj_Pause `824CF7D8` | 0, 2 | 0 unpaused | Pause-manager context mask (`824C2A50`) |
| SFXObj_NIS `824CED08` | 0, 1, 2, 4, 5, 9, 10 | 0 | Manager `+1C4` is 0 without an NIS (`824CF428` → 11); `+368` and `82475D10` need movie or FE states |
| SFXObj_Challenge `824DBC70` | 5 | 0 | Latch set only by challenge flow `825FB7A0` |
| SFXObj_Menu `824DC690` | 8 | 0 | FE state 9 only |
| SFXObj_CameraMan `824BE690` | 0 | 0 | Speech record of type 2 (`824940F0`); the port has no speech |
| SFXObj_Speech `824CFB28` | 1 | 0 | Speech entries of type `0x4B..0x4D` |
| SFXObj_Bloom `824DB318` (thunk `824DB308`) | 0 | 0 | Visual-transition fade (`827725F8`) or `cMsgFadeAmount` |
| SFXObj_VU `824DB6C0` | 1 | 0 (host adaptation) | Slew-limited audio-core bus meter; weights only the CameraMan envelope, idle without speech |
| SFXObj_Contacts | 7 | Body loop result | `824AA020` |
| SFXCTL_PlayerPhysics `8249ECA0` | 9 | 0 | Local player: group `+48` = packet `+98` bit 31; the second Player instance stays inactive |
| SFXCTL_3DColPos `8249CAD8` | 1, 3, 15 | Distance and phase per active group | Below |

`audio::main_objects` ports these updates' cone slots from explicit manager
fields; the host supplies their constructor values, because the port has no
front-end audio states, NIS, movies, challenges, speech or scripted fades.

**Distance node.** Enqueue (`82474D30`) copies the record's `+10` position by
value, and activation (`824BFC58`) links it to the group's position
controller, inside the Contacts input update. Body and grind records use
snapshot `+30`: SystemReckoning `+40`, the port's raw
`physical.reckoning.vector_64` centre of mass, not the filtered output. Deck
records use snapshot `+90`. The Collision family then publishes each active
group (`828B7C58`). Inactive groups publish nothing; their reset zeroed word
15, which disables the node.

The listener (`8247AA48`) reads context `+29D20`, the payload of the channel-7
publication component (registered at `82779C2C..82779C50`). This resolves the
host mapping left open in
[Collision position and listener-distance producers](body-impacts.md#collision-position-and-listener-distance-producers):
- Writer `8277C9F8` publishes the camera system's active record.
- Reader `8277CB90` interpolates records by the publication fraction.
- Accessor `82654C18` returns the active record.

Root `+00` is that record's position row `+30`, and root `+20` its "at" row
`+20`, normalized. The record layout matches the port's `CameraFrame`. The
port passes `CameraRuntime::presentation_frame()` position and
`basis.columns[2]`.

- Word 1 is the 3-D distance from the camera position to the record position.
- Word 3 is the X/Z angle as a 16-bit turn, measured from an origin 0.25 m
  behind the camera (`aud_general/camera` `Hash_A6F853B935E46E5F`). It is
  zero straight ahead, mirrored by side, through the `82441210` polynomial.

Node 0 (`82929F00..8292A450`) maps the distance through curve 6 between 1 m
and an upper bound of 50 m straight ahead, 40 m to either side and 35 m
behind. It blends linearly by angle between those directions and is silent
beyond both neighbouring bounds. `audio::collision_position` ports the
publication with the port's estimate-plus-refinement arithmetic.

**Results.** With free-skate inputs and the record straight ahead of the
camera, body slot 18 (and the other `-1100` slots) is:

| Camera distance (m) | 1 | 2 | 3 | 5 | 8 | 12 | 20 | 35 | 60 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Slot level | 9202 | 8668 | 8126 | 7053 | 5564 | 3984 | 1687 | 117 | 0 |
| Change from 32767 | −11.0 dB | −11.6 dB | −12.1 dB | −13.3 dB | −15.4 dB | −18.3 dB | −25.8 dB | −48.9 dB | silent |

In the stock bail replay (`raw_controller_bail_runs_stock_wipeout_for_900_ticks`
with `SKATE_OWNED_AUDIO_ROOT`), the camera is 3.40 m from the skater. The 12
body records between ticks 168 and 214 start at slot 7841 or 7850, 12.4 dB
below the former full-scale gain. Surface sides use their own class's slot.

**Validation.**
- The owned cone matches the independent `tools.audio.check_mixmap_modulation` and
  `tools.audio.check_mixmap_volume` references at ten distances.
- Native replays (`verify_main_objects.py`, local) match production Rust for:
  - NIS: 3,000 updates.
  - Pause: 3,000 sequential updates.
  - Master slots 0–6, 11 and 12 with the timer: 2,500 sequential updates.
  - Challenge: 1,000. Menu: 15. Bloom: 1,000. Speech: 1,500. CameraMan: 500.
  - The replay corrected one misreading: NIS states 2, 5 and `0x1A` select
    slot 5 (`824CF5DC`).
- `verify_collision_position.py` (local) runs `8249CAD8`, with its callees
  `8249C988`, `8249C540`, `8249BFD8` and `82441210`, in a VMX128 interpreter
  subset. The object carries its original vtable; only the controller's word
  setter and getter are hooked. Words 1, 3, 5, 6, 10, 11 and 15 match Rust
  for 4,000 publications. The listener span `8247AAD4..8247AB1C` matches for
  3,000 normalizations. Estimates follow the port's host convention, so the
  replay verifies lanes, permutes, operation order, fused rounding and
  branches, not Xenon estimate bits.
- Rust tests cover cone discovery, envelope ducking, disabled outputs (raw
  `D8F0` in the first map entry only), the Master startup pulse and an owned
  host-adapter run.
- `./play.sh --verify shot.png` logs `original collision mix loaded` and
  ends with `GAME_VERIFY_OK`.
- Cost: one MixMap phase takes a median 3.2 µs with ten active groups and
  2.9 µs with none. That is a release build on an Apple M3 Max, 50,000 phases
  per sample, median of seven. At 30 MixMap phases per second (60 Hz ticks),
  this is under 0.1 ms of CPU time per second.

**Host adaptations and gaps.**
- The original evaluator has no error path. The port rejects producer values
  outside the verified domain, such as a non-finite camera or record
  position. It then stops collision voices and drops the mix for the session
  instead of playing at a guessed level.
- The publication ratio stays at its constructed 1, as for the body loop.
  Manager `+34B`/`+34C` need publication bits from the actor's `+CC` object,
  whose writer is untraced. They stay clear, which does not matter at ratio 1.
- Pause, replay and map loads stop collision voices and reset the groups. The
  original cone instead ducks through Pause words 0 and 2 (envelopes F0.11
  and F0.15, both to −10000; `824CF7D8` picks word 0 only when manager
  `+418` is 1). Whether the audio backend also pauses voices is untraced.
- The original starts AEMS layers with gain 0.0 (`824BFE08`) and applies the
  computed gain on the next output phase. The port starts voices at the
  evaluated gain. How the backend renders that first interval is untraced.
- The listener is the current tick's rendered camera, without the channel-7
  publication interpolation.
- The deck record position uses the deck body position. That
  SkateboardReckoning `+90` maps to it is layout evidence only.
- Words 0 and 2 (the second listener) are not published; collision nodes read
  words 1 and 3.
- Pan angle (slot 0) now drives the recovered stereo branch; see
  [Collision panning](collision-pan.md#collision-panning-recovered-stereo-branch) for its
  provisional AEMS integration. Pitch slots 1 and 22 now drive voice speed
  ([details](collision-pan.md#collision-pitch-controllers)); the slot 2..11 parameter is not applied.
- **Channel-5 edge (new finding).** The body loop's channel-5 section
  (packet section A, built by `827869C0`) is present whenever scoring-object
  float `+3518` is positive. `82D82D78` writes that float as HOM worker
  `+7E0` × 1/60 on every active worker update (`82D84940`) and once when the
  wipeout ends (`82D837D4`). The worker is active for filtered category 4
  (Wipeout) with State `+44/+45` clear (`82D83620`). It counts only while
  Motion `+1C4` is clear (`82D8475C`). Section presence follows the published
  duration, including the exit packet; it cannot be equated with the current
  wipeout. An audible body region on a section rising edge sets Contacts
  word 7 = 32767 and, for a non-torso region of category 1 or more, adds
  the extra torso layer. The port now supplies the recovered duration and
  Collision `+C4` inputs ([live publication](collision-pan.md#channel-5-live-duration-publication)).
  Remaining State `+44/+45` writers and channel-5 cadence relative to the
  audio input phase still require investigation.

  In the volume cone, word 7 only triggers F1.7. That envelope adds +400 to
  sum F3.0, which is capped at 0, so it matters only while another input
  ducks the sum. The edge layer contributes one extra voice per qualifying section edge.
