# Collision gain and pitch

Native collision gain, runtime material controls, pitch and the output-record accumulator. Part of the [audio research](README.md).

## Native collision gain and runtime material controls

`skate_core::audio::voice` now implements the ordinary material-class slot
selection (824BFF88), packed unsigned 15-bit controller reads (8249D138), and
collision gain stages (824C0334..824C03DC). It preserves the float constant
38000100, single-precision multiplies and both integer truncations. Null
controllers return zero; incomplete supplied buffers fail instead of being
mistaken for a null controller. Out-of-range integer conversion is explicitly
outside the verified domain and returns an error instead of Rust saturation.

The runtime loads `body_controls` for all four primary materials and includes
them in body-event candidates. Derived base levels and controller slots must
match the original clamp and selector; pitch remains an authored integer.
Older exports may omit the controls. No live gain is inferred from these
values: the collision controller owner and its outputs are still required.

Four core tests pass. A local interpreter of the owned instruction block
matched the compiled production Rust gain for 5,140 cases, comparing both
integer stages and final float bits. Eight runtime bank tests pass, including
an opt-in owned-data test; the 12 filtered core audio tests and 18 focused
Python tests also pass. The owned-data test is run with:

```sh
SKATE_OWNED_AUDIO_ROOT="$PWD/.local/body-voice-validation" \
  cargo test -p skate-game skating_audio::materials -- --include-ignored
```

The bank was regenerated from the owned base-disc data at
`.local/body-voice-validation/material-impacts` (437273-byte manifest, 365
source clips, 337 rendered variants), validated by the runtime, then installed
at `.local/skating-audio/material-impacts` with the previous directory retained
under a `material-impacts.pre-body-voice-*` backup. Existing `surfaces`, `body`,
`body_parts` and `wheel_contacts` routes matched exactly before installation.
This makes the expanded candidate trace available without changing the
provisional event/gain playback path.

Further voice-consumer evidence: pitch lookup 824C0158 chooses slot22 for
material class9 and slot1 otherwise (material IDs >=8F take slot1 directly).
Unlike the gain accessor, virtual+38 at 8249D0A8 sign-extends a 16-bit value,
calls 82923D88, multiplies by the constant at 822F353C, then truncates. The
helper uses authored float tables at 82FBCD9C and 82FBCDD0 with 1200/100
integer decomposition and separate positive/negative arithmetic. Treating that
packed value as a linear pitch multiplier would be incorrect.

## Native collision pitch and format-1 output

`skate_core::audio::pitch` implements the pitch slot selector, signed halfword
conversion, table-based ratio and final authored-pitch combination. It shares
packed-word extraction and checked integer truncation with the gain consumer.
An absent controller returns zero directly; an existing zero-cent controller
produces 4096 before material pitch. The consumer then computes:

```text
controller_pitch = trunc_f32(table_ratio(signed_cents) * 4096)
combined_pitch = trunc_f32(controller_pitch * (authored_pitch * (1 / 4096)))
voice_pitch = combined_pitch * (1 / 4096)
```

The native constants are 4096 at 822F353C and 1/4096 at 822F35AC. Both integer
conversions retain explicit verified-range checks. The ratio helper uses 12
float entries at 82FBCD9C (48 bytes, SHA-256
`c13e2d9beaea71c36f537d765a5acf32e9ae677115e623c70bb1d0aa6c15401b`)
and 100 entries at 82FBCDD0 (400 bytes, SHA-256
`a99305d3b37ac173558b6dcef85a3660093fa59f2e41608d6ca6c5871fea335d`).
The intervening word is not a thirteenth semitone entry. Data stays in ignored
local files; the core reader accepts explicit big-endian buffers and rejects
wrong lengths, nonfinite values and nonpositive entries.

Positive ratios multiply cent, semitone and octave factors in that order.
Negative ratios preserve `(1 / semitone) / octave * (1 / cent)`, including each
single-precision rounding. A local interpreter of 82923D88..82923EA4 matched
the compiled Rust ratio for all 65,536 signed halfwords; the same run compared
controller conversion and final pitch with authored torso pitch 3796. This
includes out-of-range conversion errors rather than claiming native exception
behavior is ported.

The enabled ordinary format-1 writer is also ported. 82928860..82928894 adds
record+08, modulation-node+14 and signed write adjustment with 32-bit wrap.
It caps above 2400 and resets below -4800 to zero. The direct path at
829288CC..829289E8 adds record+08 and adjustment, caps above 2400, but clamps
below -4800 to -4800. A present modulation node with value zero therefore differs
from no node. Disabled writes and special direct-copy writes are outside this
helper. Boundary tests preserve both branches and integer overflow.

The owned collision-family routes identify the remaining upstream dependencies:
record 454C (`C1030301`) writes format1/slot1 through mapping 45D8 and arguments
`90030000`, `08000006`, `09004007`; record 459C (`C1010304`) writes format1/slot22
through mapping 4614 and argument `90030000`. These references are not assumed
to be fixed zero or host time. Their live evaluation still needs integration.

Five new pitch tests and all 17 filtered core audio tests pass. Re-running the
owned gain comparison after sharing helpers retained all 5,140 matches. Simplify
review found no further changes. These consumer/output kernels do not yet drive
runtime voices; live controller evaluation, scheduling and reset semantics
remain unresolved, so this does not close the silent-ragdoll report.

## Output-record accumulator and pitch dependency resolution

The native `audio::output::output_level` now implements
82928484..82928508. Enabled output records overwrite their prior level with the
signed high halfword of the authored value word, then add resolved scalar
values in order with 32-bit wrap. The loop uses only the low byte of the
resolved count. A null reference array leaves the freshly assigned base;
it does not retain the previous level as the shared-sum stage does. Disabled
owners overwrite the level with -10000, loaded at 82927ED0. The disabled
packed-write branch remains a separate operation.

Three Rust tests cover signed bases, null/empty arrays, owner disable, duplicate
inputs, wrapping arithmetic and counts 255/256/257. A local interpreter of the
owned instructions matched production Rust for 1,000 randomized cases,
including those count boundaries. Simplify review found no further changes.

The output references resolve through 829276A0, with the scalar path at
829276F0..82927760. The group declaration array is built in authored order at
8292A6A8; resolver family/instance/index select that array, not a declaration
whose source ID happens to match the reference. Output-reference resolution
at 82927D90..82927DA4 passes r5=1, so the scalar path returns runtime+08.
After the already verified family-instance rebinding, collision slot1's
`08000006` and `09004007` select Main-family declarations 6 and 7:

| Declaration | Authored source ID | Descriptor | Additional references |
| --- | --- | --- | --- |
| Main6, file00A4 | 08000008 | 0003ECDC | 00000034, 00000035, 00000071 |
| Main7, file00B8 | 49000025 | 0000022B | None |
| Main8, file00C0 | 48000024 | 00000000 | None |
| Main52, file0234 | 480000C0 | 00000000 | None |
| Main53, file023C | 48000071 | 00000000 | None |
| Main113, file043C | 480000C3 | 00000000 | None |

This connects Main7 to Master slot5 and Main6 through Main8 to Master slot4,
with additional authored references including HOM slots0/3. No constant-zero
or host-time substitution follows from this lookup. The scalar stage first
calls the recovered curve helper (82927F04..82927F20), then converts its linear
output using the log table. Its declaration evaluation at 82928140 onward is
the next integration dependency.

Master slots4/5 are written at 824C2F74..824C2FD0. Their source starts at 1 if
context is absent, otherwise publication header context+29070 (824C2E88).
When manager byte34B is set and that header value is below 1, the source is
multiplied by an authored setting at 824C2E94..824C2EE4. Slot5 is nonzero only
for source values above 1 and uses a separate scaling constant; slot4 follows
the bounded lower-speed conversion. Their exact constants, setting resolution
and scalar declaration arithmetic still need porting before constructing a
live collision pitch buffer. The publication header is not equated with
render delta or the requested physics timestep.
