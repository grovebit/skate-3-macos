# Collision panning and shared output

The recovered stereo branch, authored pan choices, the Pn21 ramp and the shared collision output, including channel-5 duration, suppression, pitch controllers and AEMS event starts. Part of the [audio research](README.md).

## Collision panning: recovered stereo branch

Task `task/original-audio-panning` starts from integration branch `main`,
commit `dc171c33d5d95c1f34ce8fb423f07fc3ed402bf4`. Evidence uses the base-disc
`default.xex` SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`,
and the MixMap hash recorded above; no title-update addresses are used.

The existing volume cone already carries pan. Collision output record 0 at
file `4534`, map `45AC`, executes map entry `80000000`, writing modulation
node 0's raw phase into packed slot 0. The phase comes from the queued
record position and published camera direction, as traced in
[Live collision volume controls](collision-volume.md#live-collision-volume-controls). No new
selector, camera threshold, distance scale or MixMap node is needed.

The playing collision voice reads virtual `+34`, slot 0 at `824C02D4` and
`824C0370`. That method is `824B9D10..824B9D48`: it extracts **all 16 bits**,
returns zero for a null controller/buffer, and does not mask to 15 bits as
volume does. `824C0410..824C0414` multiplies by float `822F3904`, bits
`3BB400B4` (`360 / 65535` rounded to binary32), and passes degrees at parameter
`+08` to `8294E070`. Both sides of a record receive the same angle.

AEMS update `8294EDC8` copies the six parameters, adds the layer's authored
`+10` to the angle (`8294EEA8`, `8294EED0`), and routes it through the pan
channel at `+18` (`8294EF50..8294EFA4`). Its manager template `+108C` is
looked up by four-character key **Pn21** (`8294D3AC..8294D3D4`). The recovered
Pn21 descriptor is `82FB11C8`, tag at `82FB11F8`; processing starts at
`82B01510`. Its mono positioning helper is `82AFE298`, using negative
radians-per-degree `822F370C` (`BC8EFA35`). `82B1CC68` normalizes rounded
position vectors whose square exceeds 1. The stereo output branch at
`82B1D930..82B1D9C8` takes Y, computes `a = (Y + 1) * 0.5`, `b = 1 - a`,
and normalizes `[a,b]` by `sqrt(a*a + b*b)`. Front and back centre; 90 degrees
routes to the second channel, 270 to the first. This is the recovered matrix,
not an inferred linear or trigonometric gain curve.

`audio::pan` ports that controller consumer and the unit-radius, unit-gain
mono-to-stereo branch. The host playback bridge applies its gains to body,
deck and grind-onset collision voices at startup and as the group's evaluated
pan changes. It shares clip storage, adds no extra distance attenuation and
loads one atomic gain pair per stereo frame without allocating in the audio
callback. Stereo collision exports are rejected rather than being downmixed
through an unverified matrix. The existing exporter writes mono clips.

Validation: all 65,536 phases were compared with executable instruction replays
of `82AFE298` and `82B1D930`, including `82B1CC68`. Only the sine/cosine callees
were hooked to host binary64 math, with original single-precision conversions
retained; Rust's maximum absolute channel-gain difference was `1.4901161e-7`.
Core tests exercise cardinals, the turn seam, all phases' power, high-bit
preservation and null/truncated controllers. The playback test verifies PCM
scaling, sample rate, duration, end-of-stream and gain-pair consistency when
pan changes between the two samples of a stereo frame. The core audio suite
passes 100 tests (one owned-data test skipped), the host skating-audio suite
passes 45 tests, and owned bank plus camera/MixMap integration checks pass
three tests. `cargo check -p skate-game --offline` passes. The owned camera
check keeps the queued source fixed, rotates the camera and verifies slot 0
is the unmasked producer phase. Listening in a live game remains unverified.

**Provisional integration and remaining evidence.** This does not establish
full original panning parity. The authored angle and block-ramp gaps are
narrowed in [the next investigation](#authored-collision-pan-choices-and-pn21-block-ramp). The exported offline layer mixes do not expose
AEMS per-layer angle offsets, source radius, spread, normalization settings or
independent layer voices. Pn21 also interpolates changed matrices over audio
blocks (`82B01648..82B01720`, `82B01210`); the host applies updates at stereo
frame boundaries without reproducing that block interpolation. Original
runtime trig helpers and Xenon estimate bits remain host math adaptations.
The existing host camera publication timing and immediate start-gain gaps
still apply. Wheels, rolling and other provisional foley remain centred.
Recover the authored Pn21/AEMS parameter bindings and block timing before
adding smoothing or claiming subsystem-wide parity.

## Authored collision pan choices and Pn21 block ramp

Task `task/original-pan-layer-controls` starts from integration branch `main`,
commit `8932a65aadb9eef90fb0b63224ec15655c78719a`. The original XEX hash above
was rechecked against the owned base-disc file for this investigation.

**Choice binding.** Bank registration `8294D4B0..8294D5EC` links each
36-byte leaf's layer pointer at `+20` to its authored layer list. Each layer's
`+00` becomes the pointer to choices at `+0C`, with 72-byte (`48`) stride.
Only choice byte `+02` (bank index) is overwritten in this stage; pan float
`+10` is preserved. Activation `8294DD38` selects a choice and retains its
pointer at runtime layer `+58` (`8294DE00..8294DE10`, `8294DF10`). Initial
pan (`8294EC50..8294EC78`) computes supplied parameter `+08` plus supplied
`+10` times choice float `+10`; collision supplies `+10 = 1` (`824C041C`).
Subsequent updates add choice `+10` directly (`8294EED0`). Choice float `+0C`
is a **pitch multiplier**, not the Pn21 radius (`8294EED0` is pan addition,
`8294EF24..8294EF34` uses source `+0C` in layer duration/pitch arithmetic).
Do not apply that field to the panner's radius.

`tools.owned_game.splc.event_pan_choices` now preserves every authored choice,
including repeated stream IDs with different angles. The existing offline
exporter's sample-only view delegates to this parser and retains its previous
deduplication and group/layer ordering. The new inspector reports file hashes,
choice counts, angles and exact per-event records:

```sh
python3 -m tools.audio.inspect_collision_pan /path/to/Skate_Collisions.bnk
python3 -m tools.audio.inspect_collision_pan /path/to/Skate_Collisions.bnk --event 1062
```

The full Skate_Collisions bank has 2,745 choices: 2,574 with angle 0 and
171 with offsets ±40, ±50, ±60 or ±85 degrees. However, auditing **all choices
of all variants** in the current owned collision export, not just the four
representative clips, finds:

| Bank | Exported events | Unique choice records | Nonzero angle offsets | Bank SHA-256 |
| --- | ---: | ---: | ---: | --- |
| Skate_Collisions | 163 | 955 | 0 | `4dbcd56031eab8be941ff20eddd6c99540a1f6fa67f56ee65cdc9280b5642c62` |
| Skate_Metal | 110 | 520 | 0 | `84ef7b7076fc30a00a6e369db0aa72642d02a636c22a1fc4d4915cd1122ab80c` |
| HOM_Set_1 | 7 | 90 | 0 | `4415fbe9529f6ebf34c1b5a1b3d44c38dd2456c042017e208dddbbab40a7711c` |

All 1,565 referenced choice records also have pitch multiplier 1. This closes
the authored-angle gap for the current collision event set: separate layer
voices are not needed to preserve its choice angle offsets. It does not make
the offline layer gains, timing, randomization or envelopes original, and it
does not generalize to the rest of the bank's events. Pn21 template radius,
normalization and other settings remain a separate binding investigation.

**Changed-matrix ramp.** Pn21 `82B01510` compares all ten parameters with
prior values (`+2C0..+2E4`). When unchanged, it uses the stored matrix. When
changed it copies the prior matrix to the stack, recalculates the target,
and calls `82B01210` unless the explicit initialization flag is nonzero
(`82B01648..82B01720`); initialization uses the direct matrix path.
`82B01210` subtracts prior gains from target gains and multiplies by
`822F3480`, bits `3C800000` (1/64). Its first mono channel calls `82B13988`;
additional source channels use accumulating helper `82B1C5A8`.

`82B13988` processes a fixed 256-sample channel: 64 samples of ramp, then
192 samples at a separately calculated endpoint. It constructs the first
four lane values as `prior`, `prior + step`, `fma(step,2,prior)` and
`fma(step,3,prior)`. It computes `four_step = step * 4`, forms groups with
fused four-step lane offsets, and advances the base lanes once between the
two 32-sample groups. The tail is `fma(step,64,prior)`. It must not simply
store the target: captured endpoints can differ from it by two ULPs.

`audio::pan::block::channel_gains` ports this gain calculation without heap
allocation or a new interpolation rule. Local VMX128 replays of `82B13988`
with unity input matched **all 256 words of 1,004 blocks bit-for-bit**; cache
hints/zeroing were bypassed because every output byte is subsequently written,
but no arithmetic instruction was hooked. Permanent Rust tests retain a
nontrivial replay fixture covering lane/group boundaries and the rounded tail,
as well as ramp-up, ramp-down and unchanged-matrix blocks. Python tests cover
repeated streams with distinct angles, group reuse, sample-only compatibility,
invalid events and nonfinite audit fields. Validation passes 103 core audio
tests (one owned-data test skipped), 15 pan/impact/SPLC tests and 21 impact
tool tests. The simplify review found no
additional allocation, reuse or correctness changes necessary.

**Integration still pending.** The current host panner runs before rodio's
pitch/resampling. Applying these 64 samples there would change the ramp's
duration with source pitch and rate. The new core block function therefore
has no live caller yet. Recover the original graph's output-rate/block-command
boundary and map that to a host panner **after** pitch/resampling before
connecting it. Global block alignment and first-block initialization timing
also need verification. The previously merged live pan remains as played and
approved by the user; this step does not add an arbitrary smoothing timer.

**Output clock and command boundary follow-up.** The same base-disc mapped
image (XEX SHA-256 `1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`)
contains concrete output-format evidence in constructor `82AF8888`.
`82AF8ABC` loads 48,000 into `r28`; `82AF8ACC..82AF8AE0` passes that rate and
six channels to the output object's virtual `+28` call. Independently,
`82AF8B54..82AF8BA4` builds its source format with 48,000 Hz, six channels,
32 bits, 24-byte frame alignment and 1,152,000 bytes/second. This establishes
the output format without relying on a decoded sample's metadata. The
constructor creates two `0x1800`-byte output buffers (`82AF8C38..82AF8C88`):
each holds 256 six-channel float frames. Output processing `82AF9728` passes
`0x100` frames to `82B19F60` (`82AF97F8..82AF983C`), interleaves through
`82B1E3C0` (`82AF9860`), and clamps the resulting `0x1800` bytes
(`82AF9884..82AF98DC`). The six-channel branch is evidence for the clock and
buffer size, not proof that the current host stereo matrix reproduces the
original six-channel mix or its clipping.

The output thread enters `82AF9450` through `82AF9700` and calls the shared
scheduler `82B1FDC0` (`82AF94E8`, `82AF9518`). That scheduler runs callback
list 0 via `82B202E0` (`82B1FE04`), processes graph lifecycle lists, drains
the pending command buffer (`engine +30`, byte count `+CC`,
`82B1FF28..82B1FF58`), clears its count and increments `engine +100`, then
runs callback list 1 (`82B1FFE0`). Commands are therefore executed at a shared
scheduler boundary, not at arbitrary per-voice PCM frames. Callback traversal
`82B202E0` invokes each stored function at record `+04` with its context at
`+08`. Graph creation also queues activation into this command buffer
(`82B20764..82B20788`); node constructors initialize node `+24` to zero
(`82B206C4`). This zero is not yet established as the Pn21 process function's
initialization argument.

At a 48 kHz processing rate, the recovered 64-frame ramp is 1.333 milliseconds
and a 256-frame block is 5.333 milliseconds. **Still unverified:** the graph's
rate parameter binding (output node template `+3C` is separately copied to
engine `+D8` at `82AF89CC`), the render callback that supplies Pn21's `r5`
initialization flag, and how that shared boundary maps onto Bevy/rodio's
independent sources. A live integration must resolve those bindings rather
than assume that output-device format alone proves every node's rate or
that each voice starts a new global block. The current playback bridge is
unchanged while these dependencies are traced.

**Rate binding and initialization resolved.** The static Dac0 definition is
at `82FADDAC`: its tag at `+24` is `44616330`, constructor `+08` is
`82AF8888`, parameter records `+14` point to `82FADC00`, and parameter count
`+2A` is three. The second 40-byte record (`82FADC28`) contains an authored
binary64 default of 48,000 at `+08` (`40E7700000000000`). Registration
`82B1DC68` converts numeric defaults to binary32 tagged values
(`82B1DF84..82B1DFF0`). An instruction replay with relocated copies of this
definition and its authored tables produced defaults 3, 48,000 and 0;
the rate's tagged value was `7FF7FFF1473B8000`. `82B1DBE0` copies these
8-byte defaults into the node's parameter storage: the rate tag occupies
`+38`, its float `+3C`. The constructor copies this float into engine `+D8`
at `82AF89CC`. Its block interval uses constant `822F36E8 = 43800000`
(256), dividing by the same rate (`82AF89B0..82AF89D4`). This binds the
48 kHz graph clock independently of the output device format.

The ordinary render loop supplies Pn21's initialization argument at
`82B1C280..82B1C2AC`. It loads graph byte `+45`, compares the current node
index against it using `subfc`, `eqv`, `srwi`, `addze` and `clrlwi`, and
passes **index > marker** as `r5`. Replaying those exact instructions for
all 65,536 byte-sized index/marker pairs confirmed that condition. Graph
creation clears the marker (`82B20624`). After all nodes successfully
process, the loop stores the node count (`82B1C308..82B1C314`); subsequent
ordinary blocks therefore use `r5 = 0`. This is a graph progress marker,
not node `+24` (which stores measured processing time at `82B1C2F8`).
The specialized render route also derives its flag from this marker at
`82B1BE68..82B1BEA8`. Do not replace this with a guessed first-sample timer;
a reset path also clears the marker at `82B1BCCC`.

**Host processing order preparation.** Collision playback now converts
PCM to float, applies its existing voice pitch through rodio's `Speed`,
resamples mono to the recovered 48 kHz graph rate, and then applies the
stereo matrix. Sink speed is removed for these voices so pitch is applied
once. This uses the already locked rodio 0.20.1 adapters and processes one
channel during this resampling stage; the matrix still reads one atomic
pair per stereo frame and allocates nothing per frame. Rodio's interpolation
is a host adaptation, not a port of the original Rsp0 resampler. The host
output mixer can still resample the stereo source for a device with a
different rate. No equivalence claim is made for that extra conversion.

Host tests cover 24/48 kHz inputs at 0.5/1/2 pitch, speed-adjusted duration
metadata, stereo pair consistency, end-of-source handling, and a changing
PCM fixture at 24 kHz whose mid-interpolation pan update detects the
processing order. The WAV decoder's microsecond metadata truncation is
accounted for explicitly in duration assertions.

The recovered gain ramp remains disconnected: mapping the shared scheduler
boundary to host voice insertion and update commands still requires a common
render clock. Independent 256-frame counters starting with each Bevy voice
would change cross-voice alignment and are not the original command boundary.
The rate and initialization evidence above supersedes those two missing
bindings in the preceding follow-up; shared host scheduling, full Pn21
parameter binding, and original source resampling remain open.

Validation for this preparation passes 47 host audio tests, all four owned-data
host tests, 103 core audio tests (one owned-data test skipped), and offline
checks for all three game binaries. The simplify reviewers found no necessary
reuse or efficiency changes; their quality review prompted the varying PCM
fixture above. Formatting and `git diff --check` pass. No runtime listening
comparison has been performed for this preparation.

## Shared collision output and live Pn21 ramp

The recovered 64-frame ramp is now connected to collision playback. One
persistent stereo source owns a 48 kHz, 256-frame clock for all collision
voices. The Bevy playback pass publishes its starts, stops and controller
updates as a command batch. The decoder drains commands before rendering
the next block, mirroring the shared command-before-render ordering recovered
at `82B1FF28..82B1FFE0`; individual voices no longer establish independent
block phases. The clock continues through empty collision blocks, so starting
a voice after silence does not reset it. Only an unavailable/destroyed host
output requires a new host clock.

Activation initializes the matrix directly from the latest pan parameters,
including updates queued before the first render. Later changed pan parameters
call the recovered `channel_gains` for both output channels: the first 64
frames ramp and the remaining 192 use its separately rounded endpoint.
The stored target matrix is used in the following unchanged block, matching
Pn21's stored-matrix path rather than carrying the rounded ramp tail forward.
All commands in one published batch are applied before any PCM in that block;
multiple updates to a voice therefore leave the final parameter value for
Pn21's comparison. Starts and stops take effect at this same boundary.

`CollisionControl` is now a Bevy lifetime proxy, not an individual audio sink.
The mixer signals clip completion and queued stops; the existing collision
manager retains or resets groups using those signals. Pause, replay, teleport
and group reuse submit stops through the same queue. If Bevy cannot create an
audio decoder, pending starts remain bounded by live collision groups and
updates are coalesced to their latest values. Stops reclaim pending starts.
Destroying a decoder completes both active and queued starts and marks its
output closed; playback can recreate the persistent output. This unavailable
output handling is a host adaptation, not original-game device behavior.

The change removes per-collision `AudioPlayer`/`AudioSink` instances and custom
source assets in favor of one output plus voice proxies. Commands are swapped
into a reusable buffer under the mutex, then processed after unlocking. The
voice vector reserves the ten groups' two-side capacity at decoder creation.
PCM mixing and gain ramps use fixed arrays; there is no per-frame gain atomic
read or ramp-buffer allocation. Decoder internals and voice lifetime changes
can still allocate, so this is not a claim that the entire audio callback is
allocation-free or a measured claim of overall CPU improvement.

**Scope and host adaptations.** This integrates the recovered stereo pan and
changed-matrix ramp for the currently exported mono collision event set.
The shared clock belongs to collision voices; other provisional wheel/foley
voices still use ordinary Bevy playback. Bevy's simulation publications are
batched in `PostUpdate`, and rodio/device buffering determines audible latency.
Stop and volume changes now take effect at a collision block boundary; volume
remains the existing external gain, without a claim to original Gain0 ramps.
Rodio pitch/resampling, host math helpers, stereo summation and device-rate
conversion remain adaptations. Six-channel routing, full Pn21 template binding,
original source resampling, per-layer envelopes and the unresolved body/ragdoll
impact dependencies are not completed by this change.

Tests cover overlapping voices joining an already running block clock,
commands arriving mid-block and between stereo samples, exact recovered gains
through all 256 ramp frames, direct initialization with a queued pan update,
unchanged following blocks, clip completion and queued stop ordering, silence
without clock reset, 24/48 kHz sources at 0.5/1/2 pitch, interpolation before
pan, an impulse timing fixture verifying pitch is applied once, absent output
with 10,000 updates, decoder destruction and output restart.
These exercise known original boundaries and host ownership separately; they
do not substitute for a captured full original-game audio trace.

Validation passes 54 host audio tests, all four owned-data host tests, 103 core
audio tests (one owned-data test skipped), offline checks for all three game
binaries, and formatting/diff checks. The 1,004 original VMX replay blocks
still match all 256 gain words bit-for-bit. A short actual-game verification
on Metal/Apple M3 Max loaded the 280-event owned collision bank, captured its
startup check and exited successfully with `GAME_VERIFY_OK`; this is an
integration smoke check, not a listening comparison or original-game parity
verdict. Simplify review improvements shortened command-lock lifetime,
reserved voice capacity and covered output absence/destruction explicitly.

### Settled-wipeout body audio gate

The body audio snapshot now reads `PhysicalPlayerInput.skeleton.over_599`
for `+2A5`, instead of supplying false. This connects the existing physics
producer; it adds no threshold, timer, allocation or extra physics calculation.
The record generator suppresses region records and freezes their cooldowns
only when both snapshot `+2A4` (wipeout) and `+2A5` (settled) are true.

On the owned base-disc executable (SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`),
`82D13DA0` computes wipeout byte `+1E0`, and the copy instructions
`82D13FA0..82D13FB0` in routine `82D13F58` publish it to
PhysOutSkeleton `+257` (decimal 599). The existing physical port
labels the corresponding title-update routines `82D3ED30` and `82D3EEE8`.
The base-disc calculation matches the port's strict speed/time comparisons:
hips and neck squared velocities, authored `WipeoutOverSpeed` and
`WipeoutOverMinTime`, the neck bypass after extra-weight-zero time exceeds
0.5, and a zero response scalar. Base-disc scalar `82D13658` corresponds
to the port's `82D3E5E8` calculation using response count, maximum speed,
slow time and special/below-surface flags. These are per-routine mappings,
not an assumed executable-wide address delta.

The host passes skeleton record velocities for hips (part 23) and neck
(part 1) to this existing producer. They are pose-derived positional
velocities, rather than an angular-speed proxy. Each physics output packet
resets the skeleton flag to zero before the wipeout publisher copies the
current result; audio samples the published packet after physics. Leaving
wipeout therefore clears the gate. This closes the previously hardcoded
settled input only. Channel-5 publication, Hall of Meat, owner binding,
publication-ratio writers and manager cadence remain provisional; this
change does not establish full body/ragdoll audio parity.

### Channel-5 duration adapter and native replay

`audio::hom::duration` now ports the isolated worker duration counters and
section-A presence rule. It deliberately has no host state adapter yet.
The same owned base-disc hash recorded above applies to all addresses here.
Active worker block `82D8475C..82D84790` increments unsigned word `+7E0`
only when Motion `+1C4` is clear. It additionally increments `+7E4` only
when Skeleton `+257` is clear. Both words wrap at 32 bits; worker reset
`82D821D8` clears them at `82D8220C` and `82D82214`.

Scoring publisher `82D82F30..82D82F4C` converts zero-extended word `+7E0`
to float, rounds to binary32, then multiplies by constant `82084998`
(bits `3C888889`). It writes scoring record `+4E8`, equivalent to the
scoring object's `+3518`. This is a count-derived publication value; it
does not establish the host's wall-clock update cadence. Section producer
`82786A18..82786A38` includes section A only when this value compares
greater than zero, and copies Collision `+C4` independently. Negative
zero, NaN and negative values omit the section. The Rust adapter preserves
these boundaries and copies the strength without a guessed gain curve.

The worker publishes on active updates and on exit (`82D837D4`). Its
counter reset and the scoring record are separate storage: resetting the
worker alone does not clear the published duration. Scoring reset routine `82D81E70`
clears the duration at `82D81EE8`, but its other reset callers/lifetime remain
untraced. A current-wipeout boolean therefore cannot substitute for the
channel-5 section's lifetime. The secondary worker also increments `+7E0`
at `82D842F8`; its complete dispatch is outside this isolated adapter.

`tools/audio/verify_body_audio_duration.py` compares the production Rust adapter
with 1,000 owned native instruction fixtures. All counter values, published
float words, section-presence decisions and copied strength words matched,
including unsigned/wrapping and float precision boundaries. The verifier
checks mapped-image SHA-256
`ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42`
before replay. It stops at block continuations and hooks only terminal branch
exits, with no computation hooks. It decodes the image from your own game folder:

```sh
python -m tools.audio.verify_body_audio_duration --game "/path/to/Skate 3"
```

This verification covers the isolated counter, conversion and section
selection, not the full worker or audible output. The later
[live duration publication](#channel-5-live-duration-publication) connects
Motion `+1C4` and the recovered scoring lifetime. Existing State `+44/+45` fields
already include wipeout countdown/request producers in the host; the other
accumulating suppression writers still require version/producer mapping.

### Collision C4 body contact publication

The host now publishes `PhysicalPlayerInput.collision.body_contact_strength_196`
from the original retained feedback sum. Base-disc publisher `82BAD9F0`
binds Collision table member `+18`, then its contact storage `+50` at
`82BADA08..82BADA14`. `82BAE364..82BAE368` copies feedback float `+FC4`
to storage `+74`, hence Collision `+C4`. Section-A publisher `82786A34`
copies that exact float; it is not a normalized single-region strength.
All addresses here refer to base-disc XEX SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`.

The producer runs in the existing eight-region publication loop. For each
region with a selected part, `82BAE22C..82BAE254` reads pose-derived
velocity change from SkeletonState `+FD0 + part*16`, its normal from
feedback `+3F0 + region*16`, and force from `+3B0 + region*4`. Normal
contribution uses the absolute three-lane dot product. Its gate is strictly
`dot(change, change) > 0.1` (bits `3DCCCCCD`), rather than current speed
or angular velocity. Each contribution to `+FC4` is accumulated by original
single-precision fused multiply-add at `82BAE2CC`. The parallel `+FC8`
force sum gates on force strictly greater than 0.5, and uses the separate
fused multiply-add at `82BAE2B4`. Region ordering is preserved.

The owned `physics_collision/default` schema/collection mapping is:

| Layout offset | Field | Owned float bits | Purpose |
| --- | --- | --- | --- |
| 120 | `Hash_FBF5CD7734E8AE5A` | `3F000000` (0.5) | Force-sum rate |
| 124 | `Hash_AD7ECC44569EBEF9` | `41100000` (9.0) | Normal-sum rate |
| 128 | `Hash_06C10E1F5E1B4730` | `447A0000` (1000.0) | Rate divisor |
| 164 | `Hash_1430BD50F0A33475` | `41200000` (10.0) | Independent per-region audio strength scale |

Schema records are at extracted `skaterschema.bin` offsets `1B868`,
`1B7C0`, `1B670` and `1B6A0` respectively. Their parent VLT hashes are
the schema/collection hashes recorded in the observation investigation above.
The runtime loads these authored keys and retains the division, multiply
and fused-add ordering. Rates are computed once per publication, reducing
up to eight identical divisions to one, without adding a second region pass.
The existing normal dot product also feeds the region intensity, so it is
computed once. These are operation-count changes, not a benchmark claim.

`82BAE30C..82BAE344` clamps both retained sums to 0..1 after the loop.
They survive contact release while ragdoll is set. Collision update
`82BAC640..82BAC654` clears `+FC4/+FC8/+FCC` only when its input byte
`+54` is zero; that byte is also copied to feedback ragdoll byte `+FEA`
at `82BAC404..82BAC40C`. Constructor/reset `82BAD7CC..82BAD7D8` clears
the sums. This maps to the existing TU-labelled port's `wipeout_times`
array and `SkeletonCollisionInput.ragdoll`; the first two lanes previously
remained zero because their publication producer was missing. The host
publishes after the packet reset and completed contact processing.

`tools/audio/verify_body_audio_contact_strength.py` compares the production
publication code against 1,000 finite owned instruction fixtures. Retained
sum words and all eight current-region strength words matched, including
strict threshold boundaries, missing regions, negative normal projections,
saturation, multi-axis projections, varied part weights and initial retained
values. It checks the mapped-image hash
before replay and executes the publication block without hooks. Its local
VMX interpreter uses the existing host dot3 lane convention; this is not
a Xenon hardware trace or proof of VMX rounding parity. Unit tests also
cover retained sums across contact release, leaving ragdoll and full reset.

This retained strength now feeds the duration-gated channel-5 section
through the publication described below. Remaining suppression writers and
full scoring/audio publication timing still prevent a ragdoll audio parity claim.

Validation for this producer: 107 core audio tests and 54 game audio tests
passed (one/four owned-asset tests skipped respectively). Eight skeleton
tests passed when excluding the existing
`a_moving_group_8_body_reaches_native_impact_feedback_for_a_stationary_actor`
failure. That test fails at the same group-8/impulse assertion on unchanged
integration commit `99725283470ecb301eae4d503b3c8875e193a7cc`; this task
does not change its contact-force classifier. The broader skeleton suite
therefore still has that pre-existing failure.


### Channel-5 live duration publication

The duration dependency now runs after the host's selected-state physical
publication and supplies Scoring1 `+3518` to channel-5 section A. The existing
body evaluator receives `Some(Collision +C4)` when duration is strictly
positive, including a zero strength. This enables its recovered word-8/torso
edge path without an additional host timer or a second contact accumulation.
The worker retains two integer counters and the existing gate state; no
per-frame allocation is added.

Evidence uses the base-disc executable and mapped-image hashes recorded
above, without treating title-update addresses as base-disc addresses:

- Preparation `82D8C1C0` calls `82DBA848` at `82D8C1E8`. Its
  `82DBA980..82DBA98C` block copies all `0x3950` bytes of Scoring1 from the
  template, including duration `+3518`. Constructor/reset `82DB95E0` /
  `82DB9818` resets the record at `+3030` through `82D81E70`; the duration
  zero store is `82D81EE8`. The host physical packet's default zero represents
  this template lifetime. Indirect mutations of the global template have not
  been exhaustively audited.
- Motion `+1C4` is the existing host `use_air_reckoning_452` publication.
  Wipeout output `82D14060..82D14084` tests state `+1E8`, sets the Motion byte
  and copies retained velocity to Motion `+A0`. The port already implements
  this optional retained-velocity output; its mapping is checked independently
  of the prior title-update output address.
- Worker `82D83620` updates the recovered gate, conditionally resets at
  `82D83768`, and publishes an exit at `82D837D4` before running its inactive
  path. Active `82D84308` increments at `82D8475C..82D84790` and publishes
  at `82D84940`. Inactive `82D83EF8` requires secondary gate `+8B5`
  (`82D83FA0..82D83FA8`); its tail `82D842E8..82D842F8` increments only
  total `+7E0` when Motion `+1C4` is zero, after any exit publication.
  Other inactive packets keep the template's zero output.
- Dispatcher `82DC7A60` runs the conditioner before scoring. Scoring component
  entry `82DC4B60` calls `82D78580`, which reaches the worker at `82D7874C`.
  Component reset `82DC4B58` / `82D78210` does not reset this worker. The
  host therefore adds no blanket teleport reset to its retained counters.

Three sequence tests cover entry reset, Motion and settled gates, inactive
packet zeroing, secondary-only counting, exit-before-increment and concurrent
secondary reset/exit. These test the port's composition; the existing 1,000
native/Rust fixtures verify isolated counter/conversion/section computations,
not full worker sequences. `tools/audio/verify_body_audio_scoring_template.py`
replays the full native scoring-copy block and its memcpy without hooks,
checking all bytes and duration replacement against randomized templates.

The runtime adapter remains provisional for unported accumulating State
`+44/+45` suppression writers (including material routing), and for original
provider interpolation and publication cadence relative to the alternating
audio manager. It consumes the current completed physical packet, as the host's
other audio inputs do. These gaps require further producer and timing research;
this hookup does not establish complete body or Hall of Meat audio parity.


Validation for the live hookup: 110 core audio tests, 54 game audio tests
and eight core physical input-phase tests passed. One/four owned-asset audio
tests remain skipped. The 1,000-fixture duration replay and 100-template
copy replay also passed with Python assertions disabled. Reuse, quality and
efficiency reviews found no further code changes needed. The pre-existing
skeleton group-8 test failure recorded above remains outside this change.


### Audio manager cadence and publication timing

This is additional base-disc research and verification, with **no runtime
behavior change**. The fixed-tick audio adapter, constant
`mix::PUBLICATION_RATIO`, current physical/scoring packet, host group stamp and
rendered-camera listener remain explicitly provisional. Replacing just the
clock delta would change phase cadence without reproducing its packet source.
No title-update address or constant address delta was used. Provenance:

- `default.xex` SHA-256:
  `1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`.
- Mapped image SHA-256, checked before replay:
  `ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42`.
  Virtual addresses below equal `82000000 + file offset`.

**Upstream loop and delta.** The scheduler is not always at application +0C.
That is the pending slot: `8293E6D4..8293E6E0` moves it to +08 and clears +0C.
This resolves why searching only calls through +0C missed the update.
Identity follows construction and vtables, not matching field offsets:
`8293E370` constructs the static application at `8305D1C0` and registers it
in `8302845C` (`8293E474`). `82F50A10..82F50A34` installs derived vtable
`82307314`; its +0C entry `826B71E8` calls `826B57D0`, which stores the same
object in `83027D34` at `826B5864`. The previously verified
`826DE04C..826DE05C` installs the scheduler in pending +0C; its vtable
`823071AC` +20 is `826B9860`.

Entry `8293F588` runs main loop `8293E500` at `8293F65C`.
`8293E8D8..8293E8E8` invokes active scheduler +1C, hence `826B92F8`,
which drives controller `82837968` / `82837C68` and capture `827065F8`.
A separate entry `8293F400` calls loop `8293E9D0`; thread setup
`8293EEC8..8293EF28` supplies that entry to `82A18630`.
`8293EAD4..8293EAE8` invokes active scheduler +20 with f31 as f1.
That loop measures elapsed time with `82AC55A8` at `8293EA48..8293EA50`,
then resets the timer's accumulated count/start at `8293EA58..8293EA8C`.
The timer is created with unit 5 (`8293E9E4..8293E9EC`):
`82AC5390..82AC5404` selects `8301631C`, populated with binary32
`1 / counter_frequency` at `82AC5164..82AC5168`. `82AC55A8..82AC5644`
subtracts the timer's calibrated counter overhead, converts the count to
binary32 and multiplies by that scale. This is measured **seconds**, not the
simulation timestep and not the scheduler's separate millisecond average.

`8293EA90..8293EA9C` caps the measured value at constant `82099424`,
binary32 `1.899999976158142` (bits `3FF33333`). There is no simulation-speed
multiplication in this measured-delta route. The audio manager and provider
receive that same saved value through `826B9878`, `826B9ADC`, and
`826B9BAC`. The outer call also runs render work at `827DB100`, after audio
(`826B9BC8..826B9BD0`). This establishes separate physics and audio/render
loops, **not** one audio update per physical tick, a fixed display refresh
rate, or a complete GPU-present/synchronization model. At an ideal 60 calls/s
the existing manager alternates 30 input and 30 MixMap phases; an individual
incoming delta above binary64 0.02 selects both. A 60-Hz host fixed tick cannot
represent variable measured render-loop intervals merely by renaming its delta.

**Guards and lifetime.** The outer +1D byte starts at zero (`826B8538`);
virtual +14 `826B9128..826B9144` sets it during scheduler replacement's
retirement check (`8293E674..8293E680`). Its early return skips both publication
and audio. Outer +44 is the atomic scheduler state: `826BC838` dispatches its
state machine, whose state-3 branch `826BC9A4..826BC9BC` sets it to `12`.
The publication interface's +5C is `827065A0` (vtable `823092A0`), not an audio
enable flag. It requires backing object +60 to be zero and its primary buffer's
+08/+0C to differ. These are readiness conditions, not a host pause shortcut.
Pause query `824C2A50(10)` remains the third guard; its complete mask-producer
lifecycle and the buffer's atomic-state producers are not mapped into Bevy.
Guard failure rejoins at `826B9AFC` and can still reach audio.

Audio enable is manager byte +28 (`82473074`), initialized to zero at
`82472550` and set to one at the end of initialization `82472E98`.
The immediate force producer remains `*(83027DC4) +151` (`826B9BB4`,
passed as r5), whose object-qualified writers were not established.
`827DB1B4..827DB208` also consumes this byte to select 1/2 and 60/30 render
settings; this does not establish its meaning, justify deriving it from host
frame duration, or prove that it stays zero during free skate.

**Ratio and fraction.** The ordinary provider is constructed by `82D92F10`;
its vtable `82322D00` exposes the destructor, record reader `82D93068`,
advance thunk `82D930A8` and the previously identified constant master flag. The inspected provider
methods contain no post-construction numerator write; the only positively
identified state +20 writer remains `82D92FB8` (nominal `82084998`, binary32
1/60). A whole-image `stfs +20` scan alone cannot exclude aliased/integer/vector
stores or other callers, so this is **not an exhaustive writer proof**.
No claim that the live free-skate ratio is always 1 follows from this audit.

Advance `82D92D94..82D92DE8` queries channel index before timing: nonpositive
delta skips both queries; sentinel `01FFFFFF` skips timing. Both retain +24.
Otherwise it stores the context timing query in +24 *before* checking whether
it is positive. Thus nonpositive timing replaces +24 but stops phase advance;
reader `82D93068..82D930A4` then returns zero ratio. The replay checks retained
ratios 0.5 and 2 with synthetic timing queries; these are boundary experiments,
not observed free-skate values. A skipped outer publication calls neither
provider advance nor publication dispatch, so already-published snapshot +DC
must not be rebuilt from the live simulation setting.

The channel-6 source/callback/message evidence in
[Timing publication, retained state, and update ordering](audio-manager.md#timing-publication-retained-state-and-update-ordering)
remains applicable: `82832EA0`, message `3FBF5E43`, source +7C,
`8277C2C8`, `827847F0`, context +29CD8. Reinspection of producers
`82581DB8..82581DF4` and `82D28A14..82D28A54` confirms their respective
less-than-one/byte guards and nominal-step/divisor calculation, but does not
close the divisors' entire producer chains or ordinary-play mode coverage.
The existing host `requested_timestep` therefore cannot stand in for retained
provider +24. Fraction/index tracing stops at `82D92E4C..82D92EA8`, including
`82F23CF8` and buffer traversal `82D91888`; newest-index query `82D919F8`
also depends on the buffer's +10 node and +54 table. Its empty-buffer branch
returns **zero** (`82D91A74..82D91A80`), so it must not be replaced by an
assumed sentinel on every empty read. A complete
`82D92D80` phase/index/fraction replay and host buffer implementation remain
missing. No unused partial provider implementation was added.

**Which packet, stamp and listener.** Physical/controller completion precedes
capture: `826B94A4` calls `82837C68`, then `826B94BC` calls `827065F8` with
its returned update mask. `8270660C..82706664` sends bit-0 ordinary capture
through `82D913B8`. Separately, render-side `827066C8` dispatches the
provider-selected index/fraction; `82D91558` resolves that index and an
adjacent node (`82D91610..82D91678`), and `82D928F0` constructs two callback
packet records. This is a buffered selection, not evidence for consuming the
latest completed host packet or imposing an unconditional one-tick delay.

Channel-5 registration uses `8230B58C` (`82779B60..82779BC4`); its reader
`82784618` accepts only kind 1 and forwards to `827884A0`. The latter selects
the **first callback packet** through two loads (`827884AC`, `827884C0`).
Section A `82788A74..82788AB4` checks its signed halfword offset +18 and copies
four strength bytes only when positive; absence clears presence while retaining
the stored strength. It does not use f1 or blend the second packet. This closes
the strength-interpolation question, but assigning that first packet to a
specific physics update still requires the unresolved buffer selection above.

Source `*(830734B4) +10` is initialized to zero at `82832AF4`, reset by
`82832FA0`, and incremented modulo 2^32 by update-mask bit 0 at
`82834E0C..82834E2C`; +14 independently counts bit 1. Controller
`82837968` starts +1360 at zero, sets bit 0 on a successful virtual +10 query
at `82837B30..82837B3C`, and passes the mask at `82837BC4..82837BD0`.
The reset is reached from `82837E40..82837E44`. Channel 6 captures this
stamp with its timing field, so replacing the host tick count requires the
selected packet and those reset/acceptance semantics together.

Listener reader `8277CB90` requires nonempty, matching record counts in the
two packets (`8277CBFC..8277CC28`) before interpolation. For a record with
byte +D4 set, `8277CC44..8277CC74` substitutes 0 or 1 according to descriptor
+30 bit 31; otherwise it passes the publication fraction to `828B9C10`.
The subsequent retained/override paths at `8277CCA8` and transform helper
`828B9C10` are not ported here. Interpolating the host camera blindly would
miss these conditions and would still use the wrong packet pair.

**Validation and effects.** `tools/audio/verify_audio_manager_cadence.py` takes
`--game`, rejects a mismatched executable or mapped-image hash before replay,
embeds no proprietary bytes and uses explicit failure checks
under Python `-O`. It compares **2,081 sequential native/production-Rust
clock cases**, including 60 Hz, variable deltas, threshold-adjacent values,
forced and disabled calls. Additional **native-only** research checks cover
16 publication-guard/query-order cases, 10 provider denominator/descriptor
retention cases, 15 section-A presence/copy/retention cases across five
fractions, nine upstream clamp cases and 16 mask-stamp cases including wrap.
External queries use synthetic fixtures; these checks do not replay the whole
outer update, physics/render threads, buffer scheduler or listener transform.
A new Rust test covers the 60-call alternating sequence, including the shorter
first input interval. No new production computation requires a Rust/native
comparison; the existing clock now has a repository verifier instead of only
a local one.

The unchanged host adaptations can affect cooldown/event cadence, packet age,
group ordering after resets and listener-dependent gain. This change adds zero
runtime operations, allocations or state and has no gameplay effect outside
audio (or inside it). The single constant ratio interface is preserved for
pitch and all other consumers. No performance improvement is claimed.


Final checks: core audio passed 111 tests with one owned-data test ignored;
game audio passed 54 with four ignored. With the documented owned-data paths
and `--include-ignored`, those suites passed 112 and 58 respectively, with
zero failures or ignores. Physical input-phase and skeleton-body suites each
passed eight tests; the specified pre-existing group-8 skeleton failure was
excluded with `--skip`. `cargo check -p skate-game --bins --offline`, the
verifier under Python `-O`, a deliberate bad-image rejection under `-O`, and
`git diff --check` passed. No game launch or original-hardware check was made.

### Channel-5 suppression writers

This audit adds the wipeout Collision `+D6` writer to completed State `+45`.
The owner `+760 -> +39` writer is already the host selector request and is
not duplicated. Material helper routes remain provisional and unchanged;
no new State `+41` (wants-wipeout) write is enabled. This is a partial closure
of the suppression inputs, not complete channel-5 or gameplay parity.

All addresses in this section are **base-disc** addresses. Executable
`default.xex` SHA-256 is
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`;
the inspected mapped image SHA-256 is
`ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42`.
The following mappings use code shape and field ownership independently;
no title-update address delta is used.

**Publication and dispatch.** `82D8C1C0` first copies the template through
`82DBA848` at `82D8C1E8`. Board Fill `82BDA558` is called at `82D8C284`,
then skeleton Fill `82BB9410` at `82D8C2A4`. The selector request writer
`82D8C404..82D8C428` follows. The selected owner at physical-player `+704`
is called through virtual `+24` at `82D8CC40`; wipeout constructor
`82D102C8` installs vtable `82321798`, whose `+24` entry (`823217BC`) is
`82D13F58`. Thus its suppression stores precede `82D8CF58`, the sole direct
call to final contact/material routine `82D8D420`.

That final routine dispatches on **Processed `+9CC`**, loaded at
`82D8D430..82D8D438`, not the output State `+10`. The host uses
`processed.state_2508` for the new branch. Its selected-state driver Fill,
final suppression and filtered publication now execute in that order.
The existing `body_audio_publication::advance` then reads completed
`state_flags[68-52]` / `[69-52]` for worker `82D83620`.
Native completed-dispatch entry `82D8D838` forwards owner `+764` to
`82DC7A60`; the established conditioner-before-scoring chain reaches
`82D78580 -> 82D7874C -> 82D83620`. Physical-player vtable `82322B88`
contains preparation at `+3C` and completed dispatch at `+44`. Their outer
virtual callers/frequency are still unestablished; this audit does not turn
those slot identities into proof of original-world cadence. The host keeps
one scoring advance per completed fixed-tick packet.

**Owner request (a), already present.** Constructor `82D86BC0` allocates the
small state-selector record: `82D87304..82D87348` installs its table,
processed-input and board owners, zeroes counters and bytes `+38/+39`,
and `82D87354` stores it at player `+760`. `82D8B2A8` calls selector
`82D5FF10` with that owner and the current state at `82D8B2E0`.
The selector clears `+39` at `82D5FF4C`, updates the air counter `+2C`,
and sets `+39` at `82D60164` only when the signed counter exceeds 300,
after the explicit teleport conditions at `82D60118..82D60158`.
This matches `StateSelector::calculate`: per-call request clear, category-200
counter/flag-8 hold, explicit teleport priority, then `air_frames > 300`.
Publication `82D8C418..82D8C428` conditionally writes 1 to State `+45`.
The host's existing selector request publication is this write; adding a
second writer would duplicate it. The host template clear occurs before
that publication, and the driver/final contact writes accumulate afterward.

**Skeleton contact (c), connected.** Skeleton Fill `82BB9410` passes its
contact owner (`+1930 -> +A0`) to `82BAD9F0` at `82BB9BAC`. The latter
loads contact `+FE4` at `82BADA0C` and writes Collision `+50 +86 == +D6`
at `82BADA78`. Contact update `82BAC358` clears `+FE4` at `82BAC528`,
then extracts tag bits 7..11 at `82BAC7F0` and sets it for material 6
at `82BAC804`, before ground-contact rejection. Reset `82BAD328` also
clears it at `82BAD7D0`. Direct caller `82BAFC88` is in `82BAFA00`.
These establish the mapping to existing `SkeletonCollisionFeedback::update`,
its per-update `flags.material_6` and reset; its input is the accepted
skeleton report tag, not an audio-bank material ID or a strength threshold.

The host now copies that bool to `CollisionOutputFields.material_six_214`.
When processed state is exactly 300, `82D8D4CC..82D8D4F4` reads the byte
and sets State `+45` only if the source is nonzero and the prior output
is zero. `suppression::wipeout_contact` preserves arbitrary nonzero prior
bytes; the host stores canonical bools, as it already does for State flags.
The physical-packet reset clears the new Collision field and State template
publication clears the output flags before subsequent writers. No timer,
strength calculation, report scan or allocation is added.

This has a gameplay effect outside audio: the same published byte is
`physical.state.flag_69`. Input `82D89A4C..82D89A64` routes its low bit to
Processed `+9A8` bit 18; selector `82D60128..82D60158` can consequently
select teleport. The host already has that consumer. Material-6 suppression
now participates in it, as does the existing selector/driver request.
No State `+41` behavior is changed.

**Material routes (b), still provisional.** All calls to helper `82D8D3C8`
below are inside `82D8D420`, after selected Fill. The helper's existing
207-case native research is reused: material 6 sets `+45`, materials 9/12
set `+41`, preserving existing nonzero bytes. Dispatch and producer audit:

| Processed state | Source and base-disc route | Host mapping / exact remaining evidence |
| --- | --- | --- |
| 100..104, 503 | Collision `+10`, loaded at `82D8D638`; board Fill copies CollisionInfo `+308` at `82BDAAB0..82BDAAB8` | Existing `choose_surface` is a matching four-wheel vote. Native `82BE02F0` votes from board `+2F4..+300`, contacts `+34C..+34F`, writes `+348`, and honors flag `+368` bit 25; caller `82BDFF24` is in `82BDF7F8`. Constructor `82BDDD70` embeds CollisionInfo at `+40`. Still missing: the alias installation connecting board-body `+28C` loaded at `82BDA5AC` to that embedded record, and a replay of the full wheel-source/vote/publication chain against host inputs. No Collision `+10` field is guessed from the host filtered-state surface. |
| 500, 502 | Processed left/right tags `+3E0/+410` at `82D8D564..82D8D570`; OffBoard `+132` wins over `+133` at `82D8D574..82D8D6A0`. With neither, the two material-12 tags can directly set `+41` at `82D8D6F0..82D8D714`. | Input `82D8A570..82D8A5B0` copies owner `+620/+650` tags and validity to the processed records. Host candidates are `line_tests_960_1008_1056[0..2]` and offboard publication flags. Their native query/foot-contact owner bindings and precedence have not been independently verified through the output publishers; no new wants-wipeout write is enabled. |
| 601 | Motion `+E0` tag, gated by Motion `+1C0`, at `82D8D77C..82D8D7A0` | `82D46168` copies FootPlant `+26C` to Motion `+E0`, under hit `+271`; `+1C0` is `+272 OR +273`. These match host footplant `surface`, `hit`, `perform` and `flag_627`. Producer `82D44A38` first copies result `+84` at `82D44AF8`, then calls edge replacement `82D44CD8`, which can clear the material at `82D44E58`; later invalidation clears it at `82D44B9C/82D44BF0`. Still missing: full base-disc result-owner/backend mapping and replacement-path verification against host `consume_and_submit`/`nearby_edge`. Using only the initial query material would skip those writes. |
| 602 | Processed hips tag `+440`, gated by `+444`, at `82D8D7D8..82D8D7F4` | Input `82D8A5D0..82D8A5E0` copies owner `+5F0/+5F4`. Completion `82D8B880` loops over hips/left/right records, reads query backend virtual `+20`, copies result `+84` at `82D8B900` and derives validity from result time `+30` at `82D8B904..82D8B928`; null results clear both. This matches the shape of host `SkeletonLineTests::publish` and `publish_line_tests`, but the base-disc backend's tag source and query submission/consumption binding remain unverified. Trace stops at virtual call `82D8B8D4`; it has no direct `bl` callers. |

**Driver mapping audit.** `82D10508` initializes driver word `+230` to
signed -1 (`82D106A4`) and byte `+1E4` to zero (`82D106B4`). Recovery
`82D13B50`, called at `82D110A0` in `82D10B10`, calls predicate `82D139F0`
only for negative countdowns, writes 2 on success at `82D13C34`, then
sets `+1E4` when zero or decrements a positive word at `82D13C64`.
Those reset, count and sticky-request fields map to the existing
`State::default` / `manage_recovery` / `output` fields `teleport_countdown`
and `request_teleport`. Driver Fill `82D140A8..82D140CC` tests the former
as signed nonnegative and the latter as a nonzero byte, matching the existing
`teleport_countdown_68` and `request_teleport_69` publication conditions.

The upstream predicate is **not fully equivalent**: `82D13A18..82D13A24`
tests Processed `+9B4` mask `00800000` (bit 23), whereas host
`should_teleport` tests `00400000` (bit 22). The host input currently routes
Collision byte 216 to bit 22. The corresponding base-disc input producer
and intended title-update field mapping need verification before changing
that gameplay predicate. It is now explicitly commented provisional;
its behavior is retained. Matching the final driver field tests does not
close this producer discrepancy or validate the rest of the floating-point
recovery predicate.

`tools/audio/verify_body_audio_suppression.py --game ...` checks the
mapped-image hash, replays the skeleton byte load/store and processed-state
dispatch/wipeout writer, and compares the resulting State byte and native
worker primary gate with production Rust. Its 2,268 fixtures cover 21
state IDs/signed boundaries, six source bytes, six retained bytes and three
State `+44` bytes. Exit hooks stop before the unported material branches;
no ported computation or worker primary-gate computation is hooked.
It passes with Python assertions disabled. This is not a replay of the
full contact classifier, material routes, scoring worker or outer scheduler.

Tests exercise material-6 recognition before ground filtering, accumulation
across accepted reports, source clearing on the next update, output-byte
boundaries, and an owned-data test through actual State publication and
`body_audio_publication::advance`. The latter checks packet clearing,
selector/driver/contact accumulation, processed-state dispatch and the
shared gameplay request. Existing host report filtering, fixed-tick cadence
and bool storage remain host adaptations; this change adds no new report
selection or clock. Runtime cost is one bool-to-byte copy plus at most three
scalar condition checks and a State-byte publication per packet; no runtime
performance improvement is claimed. Reusing the classifier avoids another
contact traversal and reusing the selector avoids a duplicate writer.

Validation for this writer:
- The skeleton-body suite passed 9 tests, excluding the pre-existing group-8 failure recorded above.
- `player::` passed 267 tests, and core audio passed 110 (one owned-data test ignored).
- Game audio passed 55 tests. With the owned audio, MixMap and stock-asset paths and `--include-ignored`, it passed 60, including the stock publication test.
- `cargo check -p skate-game --bins`, the verifier under Python `-O` (2,268 fixtures) and `git diff --check` passed.
- This does not check a material-6 bail on a real map.

### Collision pitch controllers

The live collision evaluator now includes output records 1 and 4 alongside
volume records 0 and 3. This supersedes the earlier zero-cent runtime pitch
status. Evidence uses only base-disc `default.xex` SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`, mapped image
SHA-256 `ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42`.
No title-update address mapping is used.

**Binding and order.** Owned `MixMapSK8.mxb` SHA-256
`105f46bbc4ae25cf51bafc8524e00b0aef608a3aa91424e3d39bbc57fff305c2` record
`454C` (`C1030301`, mapping `45D8`) resolves `90030000`, `08000006`,
`09004007`; record `459C` (`C1010304`, mapping `4614`) resolves `90030000`.
The existing binders (`8292BAA0`, resolver `829276A0`) select Main declarations
6/7, and recursively 8/52/53/113. Their file keys are `00A4`, `00B8`, `00C0`,
`0234`, `023C`, `043C`. They read Master 4/5, HOM 0/3 and Pause 1. The original
source/gate expansion and shared curve selector order remain unchanged.
Main6 reads Main8's prior curve value because its shared curve precedes Main8.

The earlier seven-declaration/two-modulation list was the union with output
record 2. **Pitch adds only six Main declarations and reuses node 0** (`4494`);
Collision declaration 0 and modulation node 1 belong to excluded record 2.
The cone now has 14 authored declarations, 20 envelope instances, 10 shared
sums, 10 modulation nodes and 40 outputs. Curves/declarations, modulation,
envelopes, sums, output accumulation and packed writes retain `82927E90`'s
stage order. No shared volume work is duplicated. Reset clears output enable;
disabled format-1 writes clear their first pitch slot without resetting shared
scalar history. Activation re-enables the existing state.

**Modulation pitch.** `8292B7A8..8292B7BC` initializes pitch and distance
history to zero. The port omits `+18/+1C` distance bookkeeping because no
selected collision output consumes it. After spatial volume, `8292A458..8292A5BC` uses the mode
curve word's low halfword (owned `0154`, 340), the selected input distance,
radial input word 13 or 14, and word 15 reset flags. For collision selector 0,
word 14 and bit 30 apply; selector 1 uses word 13 and bit 31. A reset flag is
consumed in place and selects target zero. Otherwise the ratio is
`scale / (radial + scale)`, replacing a nonpositive denominator with `scale`.
`82923820..82923D80` converts that ratio through the owned logarithm table,
using binary32 multipliers at `822F3780/822F3784`. The pitch recurrence is
`previous - trunc_f32((target - previous) * -0.2)`, with constant bits
`BE4CCCCD` at `8208EA4C`, signed word wrapping and the native truncation.
Zero scale retains pitch; disabled and far branches clear it. The format-1
writer keeps its distinct modulated/direct lower boundaries.

Collision position constructor `8249BEF8` clears its optional secondary
source `+24` at `8249BF68`. Activation `824BFC78..824BFC9C` calls the collision
vtable's `+38` setter `82557328`, which only stores source `+20`.
Publication `8249CC9C..8249CCAC` calls the radial producer `8249CCC8` only
when `+24` is nonnull. Its final stores `8249CFCC..8249D010` publish words
13/14; reset flags are set earlier in that producer. This optional source is
not bound by the traced ordinary collision activation. The live adapter
therefore retains constructed/reset radial zeros, without deriving Doppler
from host camera velocity. Other position-controller users and bindings of
that optional source are outside this collision port.

**Voice consumer and values.** `824C0158..824C01A0` selects slot 22 only for
material IDs below `8F` whose material class is 9; otherwise it selects slot 1.
`8249D0A8..8249D134` sign-extends the halfword, calls `82923D88`, scales by
4096 and truncates. `824C03D4..824C0408` combines the controller integer with
authored pitch and performs the second truncation before dividing by 4096.
An absent controller returns zero, distinct from an existing zero-cent word.
The owned 12-semitone and 100-cent tables at `82FBCD9C/82FBCDD0` are now
exported as `pitch_semitones.bin` / `pitch_cents.bin`; their hashes remain
`c13e2d9beaea71c36f537d765a5acf32e9ae677115e623c70bb1d0aa6c15401b` and
`a99305d3b37ac173558b6dcef85a3660093fa59f2e41608d6ca6c5871fea335d`.
Regenerate `mixmap/` with `tools.audio.prepare_collision_mix` for the new required
manifest entries. Validation stages the export locally; shared owned assets
are not modified.

With normal publication ratio, the first phase gives slots `(0, 0)` cents;
subsequent phases give `(-4, 0)`. Torso authored 3796 becomes 3786/4096;
deck and board grind-start material `5F` (authored 4096) become
4086/4096; truck grind-start material `60` (authored 3096) becomes 3088/4096. Those voices select slot 1, hence receive −4 controller cents.
Class-9/HOM voices select zero-cent slot 22. HOM and treatment paths are
exercised with explicit original producer inputs; the live host retains its
provisional HOM flags. `mix::PUBLICATION_RATIO` remains the single provisional
1.0 source; its upstream producers were not changed or researched here.
Record `+28` special pitch and output record 2 are excluded.

**Backend boundary and host adaptation.** Initial layer pitch is queued from
supplied parameter `+04` times layer `+40` at `8294EA5C..8294EAC4`. Later
updates calculate the same product (`8294EEC4`, stored at stack `+54`) and,
when changed, queue parameter 0 through `8294F040..8294F0A4`. The command
handler `82B1DC38..82B1DC64` writes a tagged binary32 parameter; the shared
scheduler drains commands at `82B1FF28..82B1FFE0`, before graph rendering.
The actual Rsp0 interpolation and smoothing after that parameter write remain
untraced. No new native smoothing interval is claimed.

The shared host mixer applies the latest speed at each 256-frame/48 kHz
boundary, before resampling and Pn21. A linear resampler retains its two source
samples and fractional cursor through retuning; it does not restart decoding
or duplicate pitch application. It holds the last sample through its final
fractional interval. Initial queued updates win before the first sample.
Zero/nonpositive speed holds the cursor in host silence, retaining the voice;
this is an explicit host adaptation, not a recovered zero-rate Rsp0 policy.
Linear interpolation, lack of pitch smoothing, device buffering and host
publication timing can change the audible result. There is no general
click-free parity claim: ramp fixtures show no cursor/value jump on retuning,
but an abrupt rate change can change a waveform's slope. No gameplay physics,
input or scoring behavior changes; altered audio duration changes collision
voice/group completion and can affect later audio group reuse.

**Verification.** `tools/audio/verify_collision_pitch.py --game ...` checks the
executable, mapped-image, program and table hashes before replay and fails with
explicit checks under `python -O`. It uses the native modulation constructor, native
curve and scalar loops, output accumulation and packed writer, and the native
voice consumer against production Rust over 160 persistent phases: 960 scalar
levels, 1,600 modulation states/reset flags, 3,200 pitch slot words, 6,400 voice
conversions and 160 absent-controller cases. Scalar metadata layout uses the
previously recovered constructor rules and owned attenuation table; this is
not a whole-game trace or a new allocator/binder emulation. The replay's small
interpreter extension implements missing word shifts, unsigned division and
`bdz`, without replacing audio arithmetic. Rust tests cover startup history,
reset/activation, HOM latch/treatment gates, real body/deck/grind consumers,
mid-block commands, latest-update ordering, fractional continuity, integrated
retuned duration, completion and zero-pitch lifetime.

**Cost.** Each phase adds six shared declarations and two output records per
group; the existing ten modulation nodes now also evaluate pitch. One node
lookup supplies phase, volume and pitch to each output. Host cursor increments
are calculated on retuning instead of multiplying rate and speed per sample.
There are no resampler allocations per PCM frame and no new per-voice Bevy
sinks. These are operation/allocation facts, not a measured CPU improvement.

Validation for the pitch controllers, using an export staged with the new
pitch tables:
- Core audio passed 110 tests (112 with owned data), and game audio passed
  57 (62 with owned data and `--include-ignored`).
- The input-phase and skeleton-body suites passed eight tests each,
  excluding the pre-existing group-8 failure.
- `cargo check -p skate-game --bins` and the 36 MixMap/collision Python
  unit tests passed.
- `tools/audio/verify_collision_pitch.py` passed under `-O`.

Libraries prepared before this change lack `pitch_semitones.bin` and
`pitch_cents.bin`. The runtime then rejects their `mixmap/`, and body
contacts fall back to the `routes.json` samples until
`python3 -m tools.audio.prepare_collision_mix` regenerates it.

### AEMS collision event start: choices, layers and timing

This is **research and inspection tooling only**. Collision playback still uses
its existing premixed variants; no Rust runtime or gameplay behavior changed.
All addresses below are from base-disc `default.xex` SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`, mapped
image SHA-256
`ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42`.
No title-update address mapping is used. Existing pan, scheduler and group-clock
research above is reused; live pitch-controller application is outside this track.

**Selection and retained state.** `8294DC68` selects a leaf directly, or calls
`8294F340` with group count byte `+44`, mode byte `+45` and the **group's own
first word as mutable state** (`8294DCE4..8294DD00`). The disk value `00010000`
is an initial state, not just a group type tag. Then `8294DD38` calls the same
helper per authored layer, with count `+08`, mode `+09` and state word `+04`.
Registration `8294D4B0..8294D5EC` relocates layer pointers and sample bank
indices without resetting these state words. They are shared by plays using
that bank record, rather than private to a collision group or playing voice.
A fresh bank image supplies `00010000`; activation clears the runtime event's
20 pointer slots (`8294DD50..8294DD74`), not the bank's selection histories.
Stop/destruction below does not reset those histories. The complete bank
unload/reuse lifecycle and any other state-reset writers remain untraced.

For a valid count `n` from 1 through 32:

| Mode | Native behavior |
| --- | --- |
| Any, `n = 1` | Return 0 without changing state or drawing RNG (`8294F354..8294F364`) |
| 0 | Random: truncate `f32(f32(rand * 1/32768) * f32(n))`; no history write (`8294F504..8294F548`) |
| 1 | Increment the state first, signed remainder by `n`, store and return its low byte (`8294F374..8294F3B4`) |
| 2 | Alternating-half shuffle, detailed below (`8294F3B8..8294F4F8`) |

Other nonzero modes follow the sequential branch in the executable, but the
parser deliberately retains its existing supported-bank domain of modes 0..2.
There is no weight array or generic "exclude last choice" rule in this helper.
Repeated authored choices still influence selection and must not be deduplicated.
Mode 0 can repeat; mode 2 is not a uniform permutation of the entire list.

Mode 2 interprets the high halfword as available bits and the low bit as a
half selector. With `h = n >> 1` and flag `f`, that half has size
`h + (f & n & 1)` and index offset `f*h`. It draws a start position by
truncating `(rand/32768) * (size+1)`, then scans cyclically modulo `size`
for a set bit. It clears the bit; when the mask empties it flips halves and
refills `(2^new_size)-1`. No set bit returns 0 without changing state. The
initial `00010000` therefore selects index 0 first, then enters the upper
half. For five choices and zero random draws, the indices begin
`0,2,3,4,0,1,2,3,4`. The `size+1` expression and partial first half are
intentional findings, not replaced with a textbook shuffle.

`82F23EA0` is the CRT RNG: `state = state*214013 + 2531011` modulo 2^32;
return `(state >> 16) & 0x7FFF`. Normalization uses `820300DC`, float bits
`38000000` (1/32768), so 1.0 is never returned. State is at `+14` of the
current thread's CRT record returned by `82F2D020`/`82F2CF50`; the TLS slot
index is stored at `82F95700`. Lazy CRT-record initialization writes seed 1
at `82F2CFC0`, and startup initialization does so at `82F2D1FC`.
`82F23E70` replaces this thread's seed. Direct call sites are `826CBAEC`,
`826CD948` (both take their seed from `82F24388`) and `82E60DE4` (argument
conversion through `828EC318`). This is shared across events **on the same
thread**, and also across other CRT-rand users there, not one RNG per bank.
Which reseeds run on the AEMS calling thread, indirect seed callers and the
actual interleaving with other users remain unknown. Seed 1 is not an asserted
reconstruction of a running game's audio RNG stream.

**Authored controls and start ordering.** `event_controls` in
`tools/owned_game/splc.py` retains the following fields as their decoded
binary32 values/bytes. Existing `event_pan_choices`, `event_layers` and
`event_samples` outputs are unchanged, including sample-only deduplication.

| Record / offset | Meaning and consumer |
| --- | --- |
| Leaf `+08` | Gain multiplier, applied to incoming gain by `8294E098..8294E0A0` |
| Leaf `+0C`, `+10` | Source-rate base and additive random range; one draw at event start, `8294DFC8..8294E01C`, retained at event `+58` |
| Choice `+00` | Zero-based source stream; inspection API retains the existing one-based sample IDs |
| Choice `+03` | Optional graph extension index; `FF` bypasses the callback, otherwise `8294E778..8294E7DC` binds an extra node. Its effect is not identified |
| Choice `+04` | Gain multiplier (`8294EBD8..8294EBE0`, `8294EE9C..8294EED8`) |
| Choice `+08`, `+30` | Source-rate base and additive random range, retained at runtime layer `+40/+48` (`8294E2F8..8294E328`) |
| Choice `+0C`, `+10` | Separate pitch multiplier and pan offset, as established in the pan investigation; neither is the source-rate base at `+08` |
| Choice `+14`, `+34` | Start delay and additive random delay range, retained at runtime `+50` (`8294E32C..8294E378`) |
| Choice `+18`, `+1C` | Source start/end time coordinates; `+18` initializes runtime time `+54` and feeds source setup (`8294E8FC`, `8294E9D8..8294EA3C`); `+1C` feeds release and end checks |
| Choice `+20`, `+24` | Attack end and release start time coordinates, zero disables that envelope segment (`8294F258`) |
| Choice `+28` low nibble | Curve selector used by **both** attack and release (`8294F2A8`, `8294F30C`); the other bits are not assigned a meaning |
| Choice `+2C` | Gain randomization control (`8294E28C..8294E2F4`), not a second constant gain |
| Choice `+3C` first byte | Signed scheduling priority, copied to queue record `+08`; signed comparisons in `8294D688`/`8294D928..8294D93C` |
| Choice `+40` | Activation probability (`8294DE1C..8294DE38`) |
| Choice `+44` first byte | Graph-routing branch flag (`8294E680..8294E6D0`); no stronger semantic label is claimed |

Selection is followed by a probability draw even for a single-choice layer.
The layer is accepted when `rand/32768 <= choice[+40]`; zero probability can
therefore accept a zero draw. **On rejection the native branch jumps directly
to `8294DF74`, bypassing authored-layer-pointer advancement at
`8294DF5C..8294DF70`.** The next runtime pointer slot retries the same authored
layer (including its updated choice history). It does not simply skip to the
next authored layer. Complete activation-loop replays retain this behavior.
Allocator failure instead follows the pointer-advance path. Accepted layers
retain their selected choice pointer at `+58` and routing context at `+5C`.

Event start `8294DFC8` draws the event rate once and walks the accepted runtime
layers in slot order. Each `8294E230` draws gain variation, then source-rate
variation, then a delay draw only if `choice[+34] != 0`. For `u=rand/32768`,
`x=f32(2*u-1)` and authored control `R=choice[+2C]`, the negative/zero branch
computes `1-(-x)*(1-R)`. The positive branch computes
`fma((1 / f32(1-f32(1-R)))-1, x, 1)`, with the native intermediate binary32
roundings. `R=0` has a separate positive-branch constant path at `8294E2BC`
(not covered by the start-control fixtures; no referenced choice uses it).
The resulting multiplier is held at runtime `+44`, then multiplied by choice
`+04` and incoming event gain in that order. There is no layer-count-based
attenuation in this traced gain path. The host's equal-power premix changes it.

Delay is `fma(u, range, base)` when range is nonzero. Exactly zero starts by
queuing immediately (`8294E37C..8294E3C4`); a positive remaining delay is
reduced by the incoming control packet's `+0C` elapsed time. Crossing or
reaching zero queues the layer and returns without a playing-layer update
(`8294EE04..8294EE68`). This is an audio-control-update boundary, not a
sample-exact delay renderer. Collision supplies that elapsed value from its
group `+3C` (`824C0424..824C0428`), whose seconds producer `828B7C7C` was
recovered in the group-clock investigation. A delay can therefore expire on
a later manager output phase; it must not be replaced by a fixed sample timer
without tracing the remaining queue/clock relationship.

**Envelopes, graph count and stop.** `8294F258` first tests nonzero attack end
and `current_time < attack_end`. Its normalized input is
`(current_time-source_start)/(attack_end-source_start)`. Otherwise, nonzero
release start and `current_time > release_start` use
`1-(current_time-release_start)/(source_end-release_start)`. The selected
curve multiplies the incoming gain. Thus attack has priority if authored
segments overlap; equality with either boundary does not take that segment.
`8294F558` clamps its input to [0,1]. Selector 2 passes that clamped value
through; 0,1,3,4 route through VMX helper `82461600`, with additional subtraction
and/or squaring. Their numeric approximation has **not** been replayed here;
no replacement curve or trigonometric implementation is proposed. The new
verifier covers selector 2, clamping, both boundaries and the low-nibble read.
These envelopes are evaluated on control updates, then Gai0 smooths parameter
changes at render time; they are distinct from Pn21's pan ramp.

`8294E588` builds a separate AEMS graph per accepted/started runtime layer;
`8294E874` calls `82B204D8` and `8294E87C` retains the graph pointer at `+04`.
The ordinary graph includes SnP1, Rsp0, Gai0, optional nodes, Pn21 and Sen0
(bindings `8294D2F8..8294D444`); TSt0/routing branches depend on parameters.
One event can therefore produce one source graph per accepted layer, **not
one premixed source**. A collision record may resolve two such events, one
per material side. Counts below bound authored candidates at six per event,
or twelve across two sides before probability, delay, failure or voice limits.
This is an AEMS source-graph count, not proof of a one-to-one XMA hardware-voice
allocation. Source decoding/hardware allocation remains untraced.

The pending-start queue accepts up to 40 records (`8294DBDC..8294DBF8`).
`8294D7F8` admits pending records against 60 active slots; overload takes its
signed-priority sorting/replacement path (`8294D87C..8294D9F4`). The audit
finds all referenced choice priorities zero. Complete overload ordering,
including equal-priority sort behavior, is not replayed and must precede a
runtime voice-budget port. These are shared AEMS limits, not 40/60 per collision.

Playing layers stop when the tracked time strictly exceeds
`fma(choice[+08]/runtime[+40], choice[+1C], 0.16f)` or graph byte `+47` is 2
(`8294EF14..8294EF4C`; constant `82098E00 = 3E23D70A`). The end path submits
zero controls, a source command and stop (`8294F14C..8294F218`). Stop routine
`8294E4A8` handles delayed/pending/playing states; playing-graph removal queues
`82B20AC8` and clears its node pointers (`8294E3D8..8294E488`). Event update
releases layers with state at least 4 (`8294E0DC..8294E134`), and event
liveness checks those below 4 (`8294E158`). Source loop flags, how SnP1 reaches
graph completion byte `+47`, and sample cursor advancement while gain is zero
are **not recovered**. Source metadata alone does not establish looping or
first-attack preservation. The 0.16 constant is an end guard, not a start delay.

**First render interval and gain.** Collision output `824C01B8` calls start
`824BFE08` and returns immediately when both event pointers were empty
(`824C01FC..824C020C`); gain evaluation is on a later output invocation.
Initial supplied gain is 0 (`824BFF48`). Accepted zero-delay layers enter the
pending queue; positive-delay layers retain their timer. Queue drain
`8294D7F8` → `8294DB20` → `8294E8C8` creates/initializes each source graph.
The gain command from `8294EBEC..8294EC40` targets the layer's Gai0 node.
Definition `82FAE3BC` has tag `Gai0` at `+24`, constructor `82AFB078` and
callback table `82FAE3B0`, whose process entry is `82AFB4E0`.

Gai0 current parameter is `+34`, previous target `+38`. When the render
initialization argument is nonzero, `82AFB4FC..82AFB50C` copies current to
previous before computing a step. Otherwise the step is
`f32(f32(current-previous) * 1/64)` (`82AFB510..82AFB534`, `822F3480`).
Every channel runs `82B13988`; the first 64 samples ramp and the remaining
192 use the separately rounded endpoint. Finally `82AFB58C..82AFB590`
retains the target for the next block. This is the **same arithmetic helper**
as the already ported Pn21 ramp, but a separate gain stage. The new replay
runs the complete Gai0 callback, including initialization and buffer swap,
against production Rust `audio::pan::block::channel_gains`: 98 blocks,
all 25,088 output words bit-for-bit, without arithmetic hooks.

At the established 48 kHz graph rate, a block is 5.333333 ms and the ramp
spans 1.333333 ms. Reusing the scheduler and graph-marker evidence above:
commands execute before block rendering (`82B1FF28..82B1FFE0`); the initial
flag is node-index > graph marker (`82B1C280..82B1C2AC`). Consequently:

- A graph first rendered with gain zero outputs zero through Gai0. A later
  nonzero parameter ramps from zero for 64 frames, starting at zero.
- If a nonzero gain command is already applied before the graph's first
  render, initialization uses it directly, without a zero-to-target ramp.
- If the authored envelope supplies zero at that first render, that envelope
  can still suppress the attack regardless of the caller's nonzero gain.

These are verified node-level cases, **not a fixed audible latency claim**.
The ordering of AEMS pending-start draining relative to collision output and
the output thread, and SnP1/Rsp0 cursor advancement during silent blocks,
still decide which source samples form the first audible interval. The current
host adaptation starts at evaluated gain and must remain provisional until
that ordering is recovered. Pn21 initialization/ramping does not answer this
separate gain/source question.

**Owned-bank audit.** The inspector audits every choice in every referenced
leaf, deduplicating shared records by file offset; it does not inspect only
the exporter’s four variants. `collisions.json` SHA-256 is
`7e7ad19896ca65a638535432e15818c34d7757adbdc671d1ecb7b569ee13da2d`.
Bank hashes match the three recorded in the authored-pan audit above.

| Bank | Events: leaf / random / sequential / half-shuffle | Unique leaves / layers / choices | Leaf counts by layer count |
| --- | --- | --- | --- |
| Skate_Collisions | 4 / 10 / 0 / 149 | 524 / 934 / 955 | 1:263, 2:149, 3:81, 4:25, 5:6 |
| Skate_Metal | 0 / 6 / 0 / 104 | 311 / 520 / 520 | 1:144, 2:127, 3:38, 4:2 |
| HOM_Set_1 | 0 / 0 / 0 / 7 | 22 / 90 / 90 | 2:3, 3:3, 4:8, 5:5, 6:3 |

Layer modes are respectively random/sequential/half-shuffle:
`0/926/8`, `0/520/0`, `0/90/0`. Of those layers, single-choice counts are
923, 520 and 90; their mode does not consume RNG or change history.
Skate_Collisions additionally has three two-choice, six three-choice and two
four-choice layers. This is separate from the group-level selector.

| Authored field count (unique records) | Skate_Collisions | Skate_Metal | HOM_Set_1 |
| --- | ---: | ---: | ---: |
| Leaf gain = 1 / other | 455 / 69 | 299 / 12 | 18 / 4 |
| Choice gain = 1 / other | 773 / 182 | 446 / 74 | 71 / 19 |
| Choice gain-randomization control = 1 / other | 708 / 247 | 520 / 0 | 31 / 59 |
| Base delay = 0 / positive | 318 / 637 | 327 / 193 | 25 / 65 |
| Delay range = 0 / nonzero | 786 / 169 | 514 / 6 | 44 / 46 |
| Attack-end nonzero / release-start nonzero | 50 / 93 | 32 / 131 | 5 / 13 |
| Source-start nonzero | 47 | 22 | 0 |
| Probability = 1 / less than 1 | 921 / 34 | 520 / 0 | 90 / 0 |

The 34 lower probabilities are 0.5 (8), float32 0.6 (11), and 0.75 (15).
Six Metal choices have zero authored gain. All 1,565 choices have pan offset
0, pitch `+0C` = 1, routing flag 0, extension index 0 and priority 0; extension
index 0 is **not** the `FF` bypass. Choice source-rate `+08`, leaf rate fields,
and random ranges still vary. Curve counts for selectors 0..4 are
`926/9/1/9/10`, `504/1/2/6/7`, and `90/0/0/0/0`; a stored selector is not
proof that either envelope segment is enabled. The inspector prints complete
per-value histograms for all decoded fields, including every delay and gain,
and checks references through the existing SPLC parser:

```sh
python3 -m tools.audio.inspect_collision_start --manifest /path/to/collisions.json \
  --bank-dir /path/to/owned/banks
```

Multiple `--bank-dir` arguments permit a separately extracted HOM bank;
ambiguous bank names fail. The HOM bank used here was read from the owned
`data/audio/audiofiles.big`; no bank or PCM bytes are committed.

**Verification and limits.** `tools/audio/verify_aems_selection.py` checks 12,480
native/reference selections (12,288 successive-play cases over counts 1..32
and all modes, plus 192 sequential integer-width boundaries),
4,096 RNG transitions, the seed setter, 15 complete activation loops, 18
start-control fixtures, 16 delay-expiry cases and nine linear-envelope cases.
It checks mutated bank state and RNG state, including singleton/no-draw cases,
shuffle refills, probability-zero acceptance and rejection pointer reuse.
The reference is research Python, **not a new production Rust selector**.
TLS lookup and allocation/enqueue boundaries are relocated/hooks. Shuffle's
`82F279F0` dependency is explicitly supplied as exact `pow(2,n)` for integer
`n=1..16`; that CRT math implementation is not replayed. Gain-start fixtures
use exactly representable controls 0.5, 1 and 2; non-linear envelope curves
and the gain-control-zero special case remain outside their coverage.

`tools/audio/verify_aems_start.py` checks the Gai0 renderer against the existing
production Rust ramp. Cache prefetch is ignored and cache-line clearing is
emulated; all executed arithmetic is native instruction replay. Both tools
take `--game`, check the full mapped-image SHA-256 before replay,
embed no executable bytes, and fail through explicit checks under `python -O`.
Python unit tests cover field offsets, shared group references, legacy output
compatibility, duplicate-audit input, malformed references and selector
boundaries/lifetime. Runtime and owned-data Rust suites were not changed.

**Concrete runtime port plan (not implemented).**

1. Export original event/group/leaf/layer/choice records and individual decoded
   source streams instead of premixed variants. Retain duplicates, record IDs,
   initial selection words, all decoded float words, source start/end and
   envelope controls, priorities and unresolved extension/routing bytes.
   Remove equal-power mixing, peak normalization and playback trims from the
   future original path. Keep the current export/playback intact until the
   replacement has native replay coverage and all required dependencies.
2. Keep mutable selector history per loaded bank group and authored layer,
   shared between impacts; keep the recovered thread-level RNG sequence and
   its draw order. Per playing event retain its chosen leaf and sampled rate.
   Per runtime layer retain its chosen choice, delay, sampled gain/rate,
   tracked source time, lifecycle state, graph/voice handle and prior controls.
   Do not reset bank histories on collision-group reuse. First resolve the
   seed/thread and bank-reload unknowns above rather than seed each impact.
3. Replace `EventClips` and variant-modulo selection in
   `crates/skate-game/src/skating_audio/collisions.rs` with those records.
   `CollisionBank::side` should continue material/event resolution, then
   activate the event's layers. Each accepted layer eventually owns one
   source graph/host voice: zero to six for one current exported event,
   sum of the two selected sides for a collision record. Group liveness must
   include delayed and queued layers, and finish only after all are inactive.
4. Connect admitted starts/stops and per-layer controls to the existing shared
   mixer (`CollisionOutput`/`MixerVoice`) in `skating_audio/pan.rs`, preserving its 48 kHz/256-frame command
   boundary. Add Gai0 as a separate per-layer gain stage using the verified
   64-frame helper and its own initial/previous-target state. Evaluate bank
   envelopes and delay expiry on the recovered manager control phase, not a
   guessed per-sample timer. Apply the source start/end semantics only after
   source decoding/cursor behavior and the extension node are established.
5. Before replacing playback, close `8294D7F8`'s caller/thread ordering and
   overload behavior, CRT reseeding/TLS ownership, bank-state reset writers,
   SnP1 setup/completion/looping downstream of `8294EA58` and graph `+47`,
   extension callback `8294E7DC`, and non-linear curves at `82461600`.
   Capture the zero-start → first-gain command/render sequence to decide
   whether source attacks advance silently. Then replay production Rust for
   every new computation and validate multi-event overlap, delayed layers,
   steal/stop/reuse and first-audible PCM. No gameplay behavior outside audio
   should change. The present research adds zero runtime operations; its
   decoder/audit walks each authored record once and retains shared leaf
   references, without making an unmeasured runtime performance claim.

Validation for this research:
- Both verifiers passed under Python `-O`. `verify_aems_selection.py` matched 12,480 selections, 4,096 RNG transitions and 15 activation loops. `verify_aems_start.py` matched 98 Gai0 blocks.
- 30 Python unit tests passed: the AEMS start, collision pan, impact audio, skating audio preparation, body event and collision material suites.
- The inspector reproduced the event-mode counts above from the owned banks and `collisions.json`.
- Rust code and the export are unchanged.
