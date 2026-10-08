# Audio research

Traces of the original Skate 3 (Xbox 360) audio code and authored data, and how the
port reproduces them. Records name their
evidence (executable hashes, addresses, data keys) and keep traced code,
implemented behavior and behavior verified in gameplay apart. Player-facing setup is in the
[macOS guide](../platform/macos.md#original-skating-audio); the research tools are in
`tools/audio/`.

- [Wheel-contact audio](wheel-contact.md): How the original game produces rolling and wheel-contact sounds.
- [Body-impact audio](body-impacts.md): Ragdoll and body impacts: material evidence, event selection, cooldowns, original record generation and runtime playback.
- [Collision volume](collision-volume.md): The MixMap program, envelopes, shared sums, scalar and curve evaluation, output bindings and the live volume controls.
- [Audio manager timing](audio-manager.md): Audio-family counts, the publication clock, timing publication and the native phase scheduler.
- [Collision gain and pitch](collision-gain-pitch.md): Native collision gain, runtime material controls, pitch and the output-record accumulator.
- [Board collisions](board-collisions.md): Board contact producers, grind starts and ordinary deck impacts.
- [Collision panning and shared output](collision-pan.md): The recovered stereo branch, authored pan choices, the Pn21 ramp and the shared collision output, including channel-5 duration, suppression, pitch controllers and AEMS event starts.

## Remaining parity work

The project-wide rule is to reproduce the original game's logic for every
ported feature.
This list describes known audio gaps, not a completed audit of all gameplay.
The body-impact input and selection path is connected; next come its
provisional producers. The silent-bail report remains open until a bail on a
real map is checked.

| Path | Current gap | Evidence needed before replacement |
| --- | --- | --- |
| Body/ragdoll contacts | The original record generator, groups, consumer resolution and live volume controls drive playback with `collisions.json` and `mixmap/`; several inputs are provisional ([details](body-impacts.md#runtime-body-impact-playback)) | Material/tag suppression routes and driver teleport-input mapping ([audit](collision-pan.md#channel-5-suppression-writers)); publication timing, Hall of Meat and owner producers; publication ratio writers; measured-loop/packet-buffer adapter ([details](collision-pan.md#audio-manager-cadence-and-publication-timing)); material `0x5E` fallback |
| Deck/truck impacts | Ordinary deck and grind-start records/playback connected; independent truck onset remains provisional ([details](board-collisions.md#runtime-ordinary-deck-impact-playback)) | Real-map deck/grind checks; measured-loop/packet-buffer adapter, ratio/owner producers; landing enqueue `824A8A74` |
| Wheel contacts | Recovered ordinary selector with provisional input adapter and landing guard | Original contact/surface producers, reset conditions, special modes and catch/grind behavior |
| Impact playback | Trucks and feet: host attenuation and eight voices. Body/deck/grind: original groups, consumer gain and live MixMap volume/pitch controls and recovered stereo pan | Original pitch resampling/smoothing and the slot 2..11 parameter; AEMS queue/source start timing (Gai0 64-frame ramp recovered); material-pair mix |
| Sample layers | Offline simultaneous mix with representative choices; authored controls and selection now decoded ([research and plan](collision-pan.md#aems-collision-event-start-choices-layers-and-timing)) | Runtime layer port; RNG/thread and bank-reset binding; start queue, source looping/cursor and non-linear envelopes |
| Other skating foley | Original recordings with provisional triggers and loop handling | Original rolling, grind, footstep, flip and skid controllers and their input producers |

An entry is complete only when its input, selection, timing and playback are
implemented and checked against the original evidence. Retain the distinction
between traced code, implemented behavior and behavior verified in gameplay.
