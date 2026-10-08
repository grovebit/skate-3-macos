# Audio manager timing

Audio-family counts, the publication clock, timing publication and the native phase scheduler. Part of the [audio research](README.md).

## Initialized audio-family counts and the audio-update caller

The configured counts needed by the collision sum are now traced through the
ordinary initialization path of the same base-disc executable. These are
allocated group counts, not counts of audible voices or active players.

`82472EB8` creates the fourteen family managers through `828B7488`, stores
them starting at audio manager +280, calls each present manager's virtual +8,
and then constructs the MixMap evaluator. `828B7488` selects a registered
family factory and invokes the returned manager's virtual +4 with its family
ID. The relevant registered families and creation methods are:

| Family | Registered manager | Group-creation method | Initialized groups |
| --- | --- | --- | --- |
| 0 | `CSTATEMGR_Main`, registration `82FD0FE4`, factory `824DF7B8` | Virtual +4 at `824DF8A0` creates one group, initializes its object mask to `7FFF`, and activates it with null owner | 1 |
| 1 | `CSTATEMGR_Player`, registration `82FD0FF0`, factory `824DF998` | Virtual +8 at `824DFA98` loops twice, each time initializing object mask `1FFF` | 2 |
| 3 | `CSTATEMGR_Collision`, registration `82FD0FC0`, factory `824DF258` | Virtual +8 at `824DF3A0` loops ten times, each time initializing object mask 1 | 10 |

All three call `828B7318`, which writes each new group's instance from the
manager's current +14 count (`828B7410..828B7414`), links it into the +10
list, and increments +14 (`828B7464..828B746C`). The manager constructors
initialize that count to zero. Main's virtual +8 does not create further
groups. These counts describe successful ordinary initialization; they do not
establish arbitrary modded/later lifecycle state.

At evaluator construction, `829256D0..82925718` initializes each count slot
to zero and calls the wrapper's virtual +8 for each present MixMap family.
That interface uses vtable `822F69D8`, so +8 is `824737C0`. It looks up
`*(830734E4)[4 * (family + A0)]` and returns that family manager's +14 count,
or zero when the manager is absent. Thus the evaluator receives the counts
created above before expanding its references.

Applying the existing binding reference to the actual owned collision sum
with `{0: 1, 1: 2}` yields **18 resolved references**. Both family-1 arguments
expand to instances 0 and 1; each family-0 argument binds once to instance 0.
For collision instance 0, the output's two non-modulation arguments resolve
to `30030000` and `B100004C`. Other collision instances use their own family-3
sum but the same main-instance-0 envelope. This closes the ordinary initial
count dependency; it does not supply the current values of those controls.

The immediate clock caller is also identified. At `826B9BA4..826B9BC4`,
update routine `826B9860` calls audio manager virtual +94 (`82473060`) with
f31 as f1. Its prologue preserves its incoming f1 in f31 at `826B9878`.
The force-update byte is loaded from `*(83027DC4) +151`. This byte selects
the previously recovered batching override, not an inferred host pause flag.
The enclosing update's early +1D branch (`826B9870..826B9894`) bypasses this
audio call entirely. The method is registered at vtable `823071AC` slot +20.
Constructor `826B84C0` installs that table at `826B8508..826B8510`; its callers
`826DE04C..826DE05C` and `826EAE9C..826EAEAC` install the object at application
root `*(83027D34) +0C`. The adjacent table `823071A0` has only three entries
and belongs to the different object at `83073544`. Treating these adjacent
tables as one previously misidentified the scheduler interface.

The upstream scheduler that supplies this update's f1, its pause/scaling
semantics, and the writer of the force byte remain unresolved. No host-clock
mapping is claimed from this immediate caller alone. Findings here are static
code/data evidence; gameplay playback remains unchanged and the missing
ragdoll-impact report remains open.

## Publication clock: ordinary provider and master flag

Additional base-disc tracing resolves the descriptor writer and the ordinary
provider, without establishing the host update cadence. Addresses use the same
base-disc executable/hash as the preceding investigation.

The context's +64 interface is the object stored at application root
`*(83027D34) +98`: `826B6764..826B6788` constructs it with `82705E60`, and
`826B8798..826B8800` passes it to the context constructor. Its vtable is
`823092A0`. Virtual +0C (`82706128`) installs primary and alternate providers
at +1C/+20; virtual +10 (`82E1AE90`) returns the primary provider. Channel
registration at virtual +3C (`827064C8`) forwards to `82D92A30`.

Provider setup at `8277AE40..8277AF54` branches on byte `83026AE9`. When zero,
it installs a provider constructed by `82D92F10` with channel argument 1 and
the game context, and no alternate provider. The other branch constructs a
different primary with `82725B40` and an `82D92F10` alternate with channel 2.
Do not infer the high-level mode name solely from this branch.

The ordinary provider's vtable is `82322D00`. Its virtual +04 (`82D93068`)
returns a four-word record: index, interpolation value, timing ratio, and
direction word 1. With `state = provider[+04]`, the ratio is binary32
`state[+20] / state[+24]` when the denominator is positive, otherwise zero.
Both operands initially contain `0.01666666753590107` (constant `82084998`),
so the initial ratio is 1. This is not evidence that it remains 1 in play.
The interpolation value comes separately from state +10, initialized to 1.

Provider update virtual +08 (`82D930A8`, forwarding to `82D92D80`) first
requires positive incoming f1 and a valid channel index from `82D919F8`.
It then obtains state +24 through the context's virtual +00. That method,
`8277ADB0`, reads context +29CD8. Nonpositive results stop further advancement.
The update accumulates a scaled phase, advances publication indices, and
stores the interpolation fraction at state +10. The timing publication and
caller ordering are traced below; writers of state +20 beyond construction
and the outer scheduler's cadence remain open.
The two timing fields must therefore stay explicit rather than being replaced
with the host frame delta or a hard-coded nominal ratio.

`827066C8..827067B8` gets the primary's record, applies the direction sign to
its ratio, and passes interpolation as f1 and signed ratio as f2 to
`82D91558`. That dispatcher forwards both through `82D928F0`, which stores
f2 at callback descriptor +2C (`82D92958`) and calls the channel callback with
f1 separately. Together with `827819D8`, this closes the ordinary path:

`provider timing ratio -> descriptor +2C -> abs -> header +00 -> snapshot +DC`

The positive body timer consequently decreases by at most one per body-loop
update, according to this published ratio. An authored timer of 15 is not a
15-second timer; converting it to wall time still requires the consumer's
update schedule, including pause and playback changes.

The master slot-0 helper `8277ADC0` queries primary-provider virtual +10.
For both providers constructed by the setup branch above, that entry is
`82A248A0`, which returns zero. Thus their master slot 0 publishes zero and
its curve-8 declaration produces 32767. This establishes that controller
dependency for these concrete providers only; it is not permission to replace
the master controller with a constant across untraced provider replacements.

This section records recovered behavior only. No Rust playback behavior was
changed by this trace, and silent ragdoll impacts still require runtime work
and gameplay verification.

## Timing publication, retained state, and update ordering

Context +29CD8 belongs to the channel-6 component at context +29CBC, not the
adjacent channel-5 component. `82779BC8..82779BD8` constructs channel 6 with
`82784638`, passing the same secondary context parameter saved at context +60.
`826B87A8..826B87FC` identifies that parameter as `*(830734B4)`.
The component initializes its +1C float to `0.01666666753590107`, then registers
channel 6 with size `14`. Its reader publishes a five-word packet at component
+14, placing packet +08 at context +29CD8.

The channel-6 writer `82784780` dispatches ordinary kind 1 to `8277C2C8`.
`8277C2D8..8277C2F8` builds the first three packet words from source +10,
source +94, and the float at source +7C, respectively. It serializes the
20-byte packet through `82D90AB0` at `8277C3A4..8277C3B8`.
Kind 2 uses `8277C3C8`; its timing field is instead the reciprocal of an
integer returned by application-root +10 virtual +38 (`8277C3F4..8277C434`).
The mode-dependent source must not be replaced by a single host delta.

The ordinary timing source is the object at `*(830734B4)`, allocated by
`825CE8D8` and constructed by `82832A88`. Its +7C is initialized to binary32
`1/60` at `82832B6C..82832B74`. Callback `82832EA0` replaces it with message
float +10 unless the object's virtual +00 returns true, in which case it uses
`1/60`. If the resulting value equals that default, expiry count +80 becomes
zero. Otherwise, a positive message integer +14 gives count `integer + 1`,
and a nonpositive one gives `B4` (180). It requests the rounded reciprocal
through application-root +10 virtual +34 (`82832F1C..82832F58`).
At `82835610..82835648`, a nonzero update guard r25 permits decrementing the
count; reaching zero restores +7C to `1/60` and requests integer rate 60 through
the same interface. The callback is registered for message ID `3FBF5E43` at
`82832C18..82832C8C`; message producers include `82581DC4..82581DF4` and
`82D28A24..82D28A54`, which divide the nominal `1/60` by another float before
publishing it. The message's high-level meaning, those divisors, guard
production, and update cadence still require tracing before mapping this
temporary timing change to host state.

The setter's virtual +00 is now identified as `82835820` via constructor
vtable `8230F8BC`; it reads byte `8305BD7B`. The expiry guard comes from bit 0
of `82835340`'s incoming r4 (`8283536C`), not a floating-point elapsed time.
Its direct caller `82837CA4` supplies the controller's +1360 update mask.

The host already handles camera `SimulationRateRequest` messages in
`physics/clock.rs` using separately recorded TU3 evidence. The base-disc trace
above independently establishes the unrounded field's relevance to audio;
the executable addresses are not interchangeable between versions. The host
clock now retains that unrounded `requested_timestep` alongside its integer-Hz,
100-ns timer period, including request expiry and explicit reset. This is
necessary because two requests can produce the same timer period while the
original publication distinguishes them. `SKATE_AUDIO_BODY_INPUT` reports it
as `requested_simulation_timestep`. It remains the live simulation setting,
not a claim to reproduce the retained audio publication or its update cadence.
The three focused clock tests pass, including request precision, rejected
requests preserving state, expiry, and reset. Playback selection/gain and the
provisional body onset timer are unchanged by this data-path addition.

Correction to earlier research: the ten-sample average at `826B9230` is +7C
on a **different object**, the scheduler at application-root +0C. It is not
the ordinary channel-6 source. That scheduler's timer is configured with unit
4 (`826B8570..826B8578`), selecting scale `1000 / counter_frequency` through
`82AC5390` and `82AC50E8`, and measures its timed work in milliseconds. Shared
field offsets did not establish shared identity. No runtime mapping should be
derived from the earlier rolling-average claim.

Channel-6 reader `827847F0` copies the five words unconditionally for kind 2
and sets its retained-state flag. For kind 1, `8278483C..82784878` copies only
when secondary-interface +28 (component +2C) is zero. It also stores the packet
pointer and publishes an atomic index. Thus the denominator is a published,
potentially retained timing setting, not a direct query of a live timer.

Provider advancement and audio consumption share outer update `826B9860`.
The incoming f1 is saved at `826B9878`. Publication work is conditional on
outer object +44 equaling `12`, manager virtual +5C returning true, and the
`824C2A50` query with argument `10` returning false. When those guards permit,
`826B9AD8..826B9AEC` advances the primary provider with the saved f1 (or the
preceding branch advances the alternate and forwards its record to primary
virtual +0C). It then calls `827066C8` at `826B9AF8` to publish channel data.
Later, `826B9BA4..826B9BC4` invokes audio-manager virtual +94 with that same
saved f1. Skipping publication jumps to `826B9AFC`, which still leads to this
audio call. The outer +1D early-return case skips both.

This ordering matters: a host adapter cannot assume every audio update also
advances the provider, or synthesize a fresh ratio when publication was skipped.
The previously traced audio manager can additionally alternate input-family
updates and MixMap evaluation. The body timer belongs to its actual consumer
updates, not automatically to rendering or physics ticks.

Finally, body callback kind 2 (`827810C0..82781220`) leaves header +00 intact.
Its direct stores target +970 and higher, and its copy helpers receive those
subregions. It interpolates alternate data using f1 but does not replace the
cooldown scalar written by kind 1. This closes that immediate retained-state
question; it does not establish all provider replacement/reset semantics.

These are static traces of the owned base-disc executable, not gameplay
verification. The Rust onset implementation remains provisional.

## Native audio-manager phase scheduler

`skate_core::audio::clock::AudioClock` now implements the isolated manager
scheduling calculation at base-disc `82473074..82473120`, with zero-state
construction established at `824725B0..824725BC`. The caller explicitly supplies
delta, enabled state, and force flag. Results select input-family and
MixMap/output phases with their independent binary32 elapsed values. Disabled
calls retain state; forced calls or deltas greater than binary64 0.02 select
both phases and reset alternation. Other enabled calls alternate phases,
including zero-delta calls. When both phases are selected, inputs precede
MixMap/output in the original consumer.

Four Rust tests cover alternating unequal deltas, disabled forced calls,
forced/long-update resets, threshold-adjacent values, and zero-delta phase
advancement. A local instruction interpreter executed the owned scheduling
instructions against the production Rust module for 2,000 sequential cases;
selected phases and emitted float bits matched throughout. The local verifier
is `.local/audio-investigation/verify_audio_clock_native.py`; proprietary
instruction inputs remain outside version control. This validates the scalar
scheduling kernel, not original-hardware execution or end-to-end audio.

The kernel now schedules the runtime body loop, fed provisionally with the
fixed simulation-tick delta (see
[Runtime body-impact playback](body-impacts.md#runtime-body-impact-playback)). The outer
caller's cadence, publication-provider lifetime, manager enabled/force inputs,
invalid-publication fallback (`82473448`), and the other phase consumers still
require integration. Feeding render deltas or fixed physics deltas into it
without that adapter evidence does not establish parity.

## Missing-publication fallback and counterpart class

The manager checks the provider's publication index against `01FFFFFF` at
`824731EC..824731FC`. On that sentinel it calls `82473448`, retaining the
phase decisions already made by the scheduling kernel. During an input phase,
the fallback updates only family slots 0 and 2 (manager +280/+288), rather than
all 14 families. It does not update the listener or collision-family inputs.
During a MixMap phase it still calls `82924398`, then virtual +14 on all present
family managers, followed by `82483368` on manager +30. It returns without the
normal input-phase tail `824732E8`. No valid publication therefore does not mean
silencing or freezing the entire mixer. The fallback's final helper and voice
lifetime effects still require tracing before claiming a complete adapter.

Collision event selection's counterpart class is now implemented in
`skate_core::audio::material::counterpart_class`. The event consumer bypasses
the helper for absent material `8F` and uses class 0 (`82484474..82484490`).
For other IDs, `82485750` uses an explicit 19-entry jump table for `5F..71`.
Remaining IDs below `5E`, provided they are nonnegative, read the authored
surface entry +1C. Other IDs use fallback entry `5E`; this includes jump-table
entries `6E..70`. Do not confuse these classes with physical hardness or the
material +48 class used to choose voice volume controls.

`tools/owned_game/audio_surface.py` decodes the 95 entries of
`aud_general/mapping/Hash_4CA607558B1CF440`, each 72 bytes. The impact exporter
now writes their signed +1C class values as `surface_classes`, rejecting values
outside the supported 0..2 range. Wheel export shares the same validated map
reader but continues to read its separate +08 mode word. The owned entries
contain 10 class-0, 23 class-1, and 62 class-2 values. No authored values are
embedded in the Rust implementation.

Two Rust tests cover special IDs, authored classes, fallback boundaries and
the absent-material sentinel. All 14 focused Python surface/wheel/body-event
tests pass. The Rust selector also matched owned jump-table targets and the
owned surface classes for all IDs -1 through 255 (257 cases). This validates
the selector and export, not playback. Existing playback remains provisional.
