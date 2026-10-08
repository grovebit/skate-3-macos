# Wheel-contact audio

How the original game produces rolling and wheel-contact sounds. Part of the [audio research](README.md).

## Original wheel-contact audio investigation

The previous solver-impulse gate was not the original wheel-landing trigger. A
small landing could be silent because board strength had to reach 12;
the original contact path can select a sound in its lowest category (0).
Ordinary wheel contacts now use the recovered category/group selector and
authored wheel-contact events. The impulse gate remains for body and deck/truck
collisions, whose original selection rules are still unfinished.

Evidence below is from the owned **base-disc** `default.xex`, SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`.
Addresses refer to its extracted PowerPC PE, not the TU3 addresses used
elsewhere in this repository. Earlier inspector labels in investigation logs
were 0x3C00 too high: the extracted PE is memory-mapped, not a disk-layout PE.
The addresses here have been corrected against the constructor vtable.
Proprietary binaries and disassembly remain
in gitignored `.local/audio-investigation/`.

Verified control flow:

- `8249F058..8249F08C` reads four contact flags into an audio snapshot.
  `824A6710..824A6780` compares them with four retained flags, counts newly
  contacting wheels, and takes the maximum category among those new contacts.
  Continuous support does not count as a new contact.
- `824A67EC..824A6BD4` chooses a contact event group using the number of new
  and previously latched contacts. Four new contacts use group 0, including
  category 0 (`824A68D8 -> 824A68A8 -> 824A6BE0`). No solver-impulse minimum
  appears in this branch. Other branches distinguish partial contacts; simply
  testing the player state's air-to-ground transition would miss those.
- `824A0160` classifies an input scalar into categories 0, 1, 2 using
  `aud_contacts/default` fields `Hash_2A70BB8A382574E4` (0.31) and
  `Hash_6D3D91A9BA7ADCDC` (0.5). The boundaries are inclusive for categories
  1 and 2. The input is normalized air-state time, **not impulse or height**.
  `824A0248` classifies the previously retained per-wheel scalar before storing
  the new scalar. `8249F090` obtains the new scalar by clamping snapshot `+EC`
  times 0.5 to [0, 1] while snapshot flag `+14C` is set. Producer
  `8277E9AC..8277E9B4` copies physical Air `+176` (time in state) to packet
  `+78`, which `8249EF54` copies to snapshot `+EC`. The authored Boolean
  `Hash_642CF9BFEC6BE988` enables this path; the alternative reads packed
  categories from the incoming packet.
- `824A8288` indexes an authored event array with `3 * group + category`.
  `824A81A8` selects among four modes using a surface-map value and a separate
  wheel-hardness predicate. The surface-map value comes from offset 8 of
  `aud_general/mapping/Hash_4CA607558B1CF440` entries (`82482BB8`). The second
  predicate is wheel hardness < 0.5: `8277F318..8277F340` compares Motion
  `+200` against 0.5, and `82BDB0CC..82BDB0D4` publishes it from processed
  input `+2764` (wheel hardness). The original also has remote-listener overrides. Some category-2/group combinations force
  event mode 0 while preserving the original mode for later gain selection.
- `824AC918` uses the same group/category index to read a mode-specific gain
  array and multiply the voice's base volume. The gains are authored separately
  from sample selection, rather than derived solely from solver impulse.

| Mode | Event array hash | Gain array hash |
| --- | --- | --- |
| 0 | `5A93802D11B00173` | `CEA5AFA8BA170B07` |
| 1 | `A0F86FEEA9C2412F` | `951AD53718030327` |
| 2 | `797EC34502499EC3` | `068C8C5EBEC1B45F` |
| 3 | `AB0D92058B4293A7` | `6823F4910A1882AD` |

All four event arrays have 13 entries; their gain arrays have 15. Preserve the
actual lengths instead of assuming that every gain slot names an event. For
example, mode 0/group 0/category 0 selects event `0x41B` with authored gain
0.84, whereas category 1 selects `0x41C` with gain 1.0. The exporter now retains all four
contact tables and their gain arrays, including null event slots redirected by
the native selector. It still chooses representative `aud_material` events for
other collisions.

The runtime adapter uses completed physical wheel contacts, the physical air-state
clock, wheel hardness, and the retained audio surface of wheel 0. It primes
contact history after loads/teleports and ignores stale solver frames. Wheel
rows no longer feed the generic impulse trigger; deck/truck and body collisions
still do. Missing old-library contact tables keep the previous fallback path.
Wheel-event volume multiplies the authored table by the existing per-clip
attenuation and 0.65 host voice level. The original also multiplies by a
dynamic base voice volume: its packed 15-bit parameter 3 is read through the
voice object, not a constant 1.0. That controller and the original sample
envelopes remain unported; removing the host attenuation made contacts too loud.

A temporary host adapter guard arms wheel landings only when the physical
air-state family is active and all four wheels leave contact. It preserves
partial-contact grouping until landing completes, then suppresses rolling
contact jitter. There is no minimum air time, height or impulse. This guard is
not a recovered original rule, and deliberately leaves grounded wheel taps
silent until the original contact-input producer is reconstructed.

This is a port of the ordinary wheel-contact path, not complete original audio
parity. The adapter gates air time with the host air-state family; original
packet flag +14C comes from physical Air +438. Special-mode overrides, catch
sounds, the grind-specific scalar reset, full per-sample timing/envelopes and
randomizers, and original body/material-pair mixing remain unfinished. Wheel
material comes from actual wheel-0 solver contacts rather than the original
retained line-query audio surface producer. These remaining adapter differences
must not be presented as verified equivalents. Body collisions retain a separate
provisional path; these findings do not establish their thresholds.
