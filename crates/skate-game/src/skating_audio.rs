//! Original decoded samples, driven by completed simulation ticks.
//! Wheel contacts use recovered event/category tables. With `collisions.json`
//! and its `mixmap/`, head, torso, arm and leg impacts come from the original
//! body loop, fed through the PlayerPhysics speed graph, and the original deck
//! loop and grind onset are connected. Collision voice volume follows the
//! original MixMap controls; their pitch controllers and other foley remain
//! provisional.
use crate::{
    app::SimulationSet,
    physics::{GamePhysics, SkaterRuntime},
};
use bevy::{
    audio::{AddAudioSource, AudioSinkPlayback, Volume},
    prelude::*,
};
use skate_core::audio::{body_impact, clock::AudioClock, deck_impact, grind_start, player_physics};
use skate_core::player::state::PhysicalStateId as State;

mod body_material;
mod collisions;
mod impact;
mod loops;
mod materials;
mod mix;
mod pan;
mod wheels;

pub(crate) struct SkatingAudioPlugin;
impl Plugin for SkatingAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<pan::CollisionSource>()
            .init_resource::<pan::CollisionOutput>()
            .init_resource::<Mix>()
            .init_resource::<BodyAudio>()
            .add_systems(Startup, load)
            .add_systems(FixedUpdate, sample.after(SimulationSet::Physics))
            .add_systems(PostUpdate, playback);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sound {
    Roll,
    Grind,
    Flip,
    Step,
    Skid,
    Impact,
}
impl Sound {
    fn bank(self) -> &'static str {
        match self {
            Self::Roll => "Rolling_Rattles",
            Self::Grind => "GRINDS",
            Self::Flip => "Sk8_Air_Flip_Tricks",
            Self::Step => "fstep_skateshoe1_sm",
            Self::Skid => "WHEEL_SKID_BANK",
            Self::Impact => "Skate_Collisions",
        }
    }
    fn subsongs(self) -> &'static [usize] {
        match self {
            Self::Roll | Self::Grind => &[1],
            // 1..4 have slow, noise-like attacks. These four are short
            // attack transients from the same unmodified retail bank.
            Self::Step => &[139, 140, 141, 142],
            _ => &[1, 2, 3, 4],
        }
    }
    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Resource)]
struct Samples {
    clips: [Vec<Handle<AudioSource>>; 6],
    volume: f32,
    materials: materials::Materials,
    collisions: Option<collisions::CollisionBank>,
}
#[derive(Component)]
struct Voice;
/// Voices owned by the ten original collision groups.
#[derive(Component)]
struct CollisionVoice;

/// Original body/deck/grind state. Only constructor 824A5B70 resets the timers; host
/// pause, replay and teleport stops reset just the collision groups.
#[derive(Resource, Default)]
struct BodyAudio {
    clock: AudioClock,
    /// Snapshot +D8. The original also resets it with the Player group
    /// (824E6A78); the host has no producer for those triggers, so it lasts
    /// as long as this resource, like the timers.
    player_physics: player_physics::PlayerPhysics,
    body: body_impact::BodyImpacts,
    deck: deck_impact::DeckImpacts,
    grind: grind_start::GrindStarts,
    /// Group+3C: written by input updates, read by output updates.
    input_elapsed: f32,
    /// Host stand-in for publication source +10, which 824E6728 stamps.
    stamp: u32,
    groups: collisions::Groups,
    /// Voices of groups reset for reuse, stopped by the next playback pass.
    stopping: Vec<Entity>,
    /// Live collision volume controls, present while collision playback runs.
    mix: Option<mix::MixState>,
    failed: bool,
}
impl BodyAudio {
    /// 82474D30 hands the record to 824DF400/824E6728, which reset a reused
    /// group and activate it (824BFC58) inside the Contacts input update.
    fn enqueue(&mut self, record: body_impact::Record, position: [f32; 4]) {
        let Some((group, reset, stolen)) = self.groups.allocate(self.stamp, record, position)
        else {
            return;
        };
        self.stopping.extend(stolen);
        if let Some(mix) = self.mix.as_mut() {
            if reset {
                mix.reset_group(group);
            }
            mix.activate_group(group);
        }
    }

    /// Host stops: every group is reset with its voices.
    fn reset_groups(&mut self) {
        self.groups = default();
        if let Some(mix) = self.mix.as_mut() {
            for group in 0..collisions::GROUPS {
                mix.reset_group(group);
            }
        }
    }
}
#[derive(Resource, Default)]
struct Mix {
    tracker: Tracker,
    pending: Vec<(Sound, f32, impact::Contact)>,
    loops: [Option<LoopVoice>; 2],
    variant: usize,
    reset_voices: bool,
}

#[derive(Clone, Copy)]
struct LoopVoice {
    entity: Entity,
    envelope: loops::Envelope,
}

#[derive(Clone, Copy)]
struct Frame {
    state: State,
    position: Vec3,
    speed: f32,
    generation: u64,
    discontinuity: bool,
    trick_sequence: u32,
    contacts: impact::Contacts,
    wheels: Option<wheels::Input>,
    wheel_surface: u8,
    dt: f32,
}
#[derive(Default)]
struct Tracker {
    previous: Option<Frame>,
    stride: f32,
    desired: Option<Sound>,
    speed: f32,
    reset: bool,
    impacts: [impact::Onset; 6],
    wheels: wheels::Tracker,
    impact_events: Vec<(f32, impact::Contact)>,
}
impl Tracker {
    fn update(&mut self, frame: Frame) -> Vec<Sound> {
        let mut events = Vec::new();
        self.impact_events.clear();
        let previous = self.previous.replace(frame);
        self.reset = false;
        self.speed = if frame.speed.is_finite() {
            frame.speed.clamp(0., 30.)
        } else {
            0.
        };
        self.desired = None;
        let Some(old) = previous else {
            if let Some(input) = frame.wheels {
                self.wheels.prime(input);
            }
            return events;
        };
        let distance = frame.position.distance(old.position);
        if frame.discontinuity
            || frame.generation != old.generation
            || !distance.is_finite()
            || distance > 2.0
        {
            self.stride = 0.;
            self.impacts = default();
            self.wheels = default();
            if let Some(input) = frame.wheels {
                self.wheels.prime(input);
            }
            self.reset = true;
            return events;
        }
        if let Some(input) = frame.wheels {
            for event in self.wheels.update(input).into_iter().flatten() {
                self.impact_events.push((
                    1.,
                    impact::Contact {
                        wheel: Some(event),
                        audio_surface: frame.wheel_surface,
                        ..frame.contacts[0]
                    },
                ));
            }
        }
        for (onset, contact) in self.impacts.iter_mut().zip(frame.contacts) {
            if let Some(gain) = onset.update(contact, frame.dt) {
                self.impact_events.push((gain, contact));
            }
        }
        // Keep the two strongest distinct impact families in a crowded bail.
        self.impact_events
            .sort_by(|a, b| b.1.strength.total_cmp(&a.1.strength));
        self.impact_events.truncate(2);
        events.extend(self.impact_events.iter().map(|_| Sound::Impact));
        if self.speed > 0.3 {
            self.desired = if frame.state.is_grind() {
                Some(Sound::Grind)
            } else if matches!(frame.state, State::PhysicsGround | State::GroundAnimation) {
                Some(Sound::Roll)
            } else {
                None
            };
        }
        if matches!(frame.state, State::SlideGround | State::RevertGround)
            && frame.state != old.state
            && self.speed > 0.5
        {
            events.push(Sound::Skid);
        }
        if frame.state.category() == 200 && frame.trick_sequence != old.trick_sequence {
            events.push(Sound::Flip);
        }
        // Distance-based provisional footfalls. Authored animation foot markers
        // and surface-specific sample selection are not yet wired to audio.
        if frame.state == State::BipedGround && old.state == State::BipedGround {
            let horizontal = (frame.position.xz() - old.position.xz()).length();
            if frame.dt > 0. && horizontal / frame.dt >= 0.15 {
                self.stride += horizontal;
            } else {
                self.stride = 0.;
            }
            if self.stride >= 0.85 {
                self.stride %= 0.85;
                events.push(Sound::Step);
            }
        } else {
            self.stride = 0.;
        }
        events
    }
}

fn load(
    config: Res<crate::config::Config>,
    mut commands: Commands,
    mut assets: ResMut<Assets<AudioSource>>,
    mut body_audio: ResMut<BodyAudio>,
) {
    let root = std::env::var_os("SKATE3_AUDIO")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| config.asset_root.join("private/skating-audio"));
    if !root.is_dir() {
        return;
    }
    let volume = std::env::var("SKATE3_AUDIO_VOLUME")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(0.45)
        .clamp(0., 1.);
    let mut clips: [Vec<Handle<AudioSource>>; 6] = default();
    for sound in [
        Sound::Roll,
        Sound::Grind,
        Sound::Flip,
        Sound::Step,
        Sound::Skid,
        Sound::Impact,
    ] {
        for &index in sound.subsongs() {
            let relative = format!("{}/{index:04}.wav", sound.bank());
            // Validate before Bevy's decoder sees the bytes; a damaged optional
            // library must not panic on the audio thread.
            let bytes = skate_mods::read_bounded(&root, &relative, 2 * 1024 * 1024).ok();
            let Some(bytes) = bytes.and_then(|b| {
                skate_mods::audio::canonical_pcm_wav(&b)
                    .ok()
                    .map(|(pcm, _)| pcm)
            }) else {
                warn!(
                    "Skating audio: missing or invalid {} clip {index}; prepare the audio library again",
                    sound.bank()
                );
                continue;
            };
            clips[sound.index()].push(assets.add(AudioSource {
                bytes: bytes.into(),
            }));
        }
    }
    info!(
        "Skating audio: loaded {} original clips, volume {volume}",
        clips.iter().map(Vec::len).sum::<usize>()
    );
    let materials = materials::Materials::load(&root, &mut assets);
    let collisions = collisions::CollisionBank::load(&root, &mut assets).and_then(|(bank, data)| {
        match mix::MixState::new(data) {
            Ok(state) => {
                info!("Skating audio: original collision mix loaded");
                body_audio.mix = Some(state);
                Some(bank)
            }
            Err(error) => {
                warn!("Skating audio: collision mix rejected ({error:?}); body impacts stay provisional");
                None
            }
        }
    });
    commands.insert_resource(Samples {
        clips,
        volume,
        materials,
        collisions,
    });
}

fn sample(
    physics: Res<GamePhysics>,
    time: Res<Time<Fixed>>,
    skater: Res<SkaterRuntime>,
    map: Res<crate::world::map_transition::CurrentMap>,
    menu: Res<crate::menu::graphics_menu::Menu>,
    replay: Res<crate::replay::Replay>,
    transition: Res<crate::world::map_transition::MapTransition>,
    samples: Option<Res<Samples>>,
    mods: Option<Res<crate::modding::Mods>>,
    camera: Res<crate::camera::CameraRuntime>,
    mut mix: ResMut<Mix>,
    mut body_audio: ResMut<BodyAudio>,
    mut contacts: Local<impact::FreshContacts>,
    mut wheel_surface: Local<(u64, u8)>,
) {
    let fresh_contacts = contacts.observe(map.generation, physics.board.solve_generation());
    if samples.is_none()
        || menu.open
        || replay.active
        || transition.busy()
        || physics.failed
        || mods.as_deref().is_some_and(|m| {
            crate::modding::player_attached(m) || crate::modding::player_suspended(m)
        })
    {
        mix.tracker = default();
        mix.pending.clear();
        return;
    }
    let p = skater.centre_of_mass_output.position;
    let wheel_audio = samples
        .as_ref()
        .is_some_and(|samples| samples.materials.has_wheels());
    let deck_audio = samples
        .as_ref()
        .is_some_and(|samples| samples.collisions.is_some());
    let (mut contacts, surface) = if fresh_contacts {
        impact::contacts(&physics, &skater, wheel_audio, deck_audio)
    } else {
        default()
    };
    if fresh_contacts {
        impact::trace_body_audio(
            &physics,
            &skater,
            &contacts,
            samples.as_ref().map(|s| &s.materials),
        );
    }
    if let Some(bank) = samples.as_ref().and_then(|s| s.collisions.as_ref()) {
        // Head, torso, arm and leg channels belong to the original body loop,
        // and deck reports, excluded by the contact observer, to the deck
        // loop. Trucks and feet retain their provisional paths.
        for contact in &mut contacts[1..=4] {
            *contact = default();
        }
        // Listener root +00/+20: the rendered camera's position and "at" row.
        let listener = camera.presentation_frame().map(|frame| {
            let at = frame.basis.columns[2];
            (frame.position, [at[0], at[1], at[2], 0.0])
        });
        let deck = physics.board.bodies()[skate_core::physics::board::BodyId::Deck.index()]
            .rates
            .position;
        if body_audio.mix.is_some() {
            update_body(
                &mut body_audio,
                bank,
                &skater,
                time.delta_secs(),
                listener,
                [deck.x, deck.y, deck.z, 0.0],
            );
        }
    }
    if wheel_surface.0 != map.generation
        || skater.player_input.processed.flags_2472 & (1 << 10) != 0
    {
        *wheel_surface = (map.generation, 0);
    }
    if let Some(surface) = surface {
        wheel_surface.1 = surface;
    }
    let events = mix.tracker.update(Frame {
        state: skater.player_state.current(),
        position: Vec3::new(p[0], p[1], p[2]),
        speed: physics.riding.motion.speed,
        generation: map.generation,
        discontinuity: skater.player_input.processed.flags_2472 & (1 << 10) != 0,
        trick_sequence: skater.scoring.trick_sequence(),
        contacts,
        wheels: fresh_contacts
            .then(|| {
                samples.as_ref().and_then(|samples| {
                    let state = skater.player_state.current();
                    // Physical air output +176 feeds base-disc packet +78.
                    samples.materials.wheel_input(
                        std::array::from_fn(|i| physics.riding.ground.parts[i].in_contact),
                        skater.player_input.physical.air.time_in_state_176,
                        matches!(
                            state,
                            State::PhysicsAir | State::KnownAir | State::PhysicsAirSecondary
                        ),
                        wheel_surface.1,
                        skater.player_input.processed.scalar_2764,
                    )
                })
            })
            .flatten(),
        wheel_surface: wheel_surface.1,
        dt: time.delta_secs(),
    });
    if mix.tracker.reset {
        mix.reset_voices = true;
        mix.pending.clear();
    }
    let mut impact_index = 0;
    for event in events {
        let (gain, contact) = if event == Sound::Impact {
            let hit = mix.tracker.impact_events[impact_index];
            impact_index += 1;
            hit
        } else {
            (if event == Sound::Step { 0.7 } else { 1. }, default())
        };
        if mix.pending.len() < 4 {
            mix.pending.push((event, gain, contact));
        }
    }
}

/// One audio-manager update (82473060). Inputs without a ported producer keep
/// provisional values (docs/audio/body-impacts.md): the clock is driven by the
/// host tick, the publication ratio stays at its constructed 1, the current
/// physics publication stands in for the selected packet, and the Hall of
/// Meat and owner inputs are off.
fn update_body(
    audio: &mut BodyAudio,
    bank: &collisions::CollisionBank,
    skater: &SkaterRuntime,
    delta: f32,
    listener: Option<([f32; 4], [f32; 4])>,
    deck_position: [f32; 4],
) {
    audio.stamp = audio.stamp.wrapping_add(1);
    let phases = audio.clock.advance(delta, true, false);
    if let Some(elapsed) = phases.inputs {
        audio.input_elapsed = elapsed;
        input_phase(audio, bank, skater, elapsed, listener, deck_position);
    }
    if let Some(elapsed) = phases.mixmap {
        // Evaluator 82924398 precedes the output families' updates.
        if let Some(Err(error)) = audio.mix.as_mut().map(|state| state.advance(elapsed)) {
            // A producer value outside the verified domain leaves the
            // evaluator unusable; collision playback stops instead.
            warn!(
                "Skating audio: collision mix update rejected ({error:?}); collision voices stop"
            );
            audio.mix = None;
            let voices: Vec<_> = audio.groups.voices().map(|(_, voice, _)| voice).collect();
            audio.stopping.extend(voices);
            audio.groups = default();
            return;
        }
        audio.groups.evaluated();
        let _ = audio.grind.output(audio.input_elapsed);
    }
}

/// Input phase: listener and Main family, then the Player group: its
/// PlayerPhysics publication (8249ECA0) before its Contacts update (824A60B0:
/// 824A6F70, body 824AA020, deck 824AAE98) with immediate group activations,
/// as 828B7C58 updates controllers before objects. Then 3DColPos for active
/// collision groups.
fn input_phase(
    audio: &mut BodyAudio,
    bank: &collisions::CollisionBank,
    skater: &SkaterRuntime,
    elapsed: f32,
    listener: Option<([f32; 4], [f32; 4])>,
    deck_position: [f32; 4],
) {
    if let Some(state) = audio.mix.as_mut() {
        state.publish_main(elapsed, listener);
    }
    // Snapshot +30 (packet +00): SystemReckoning +40, the raw centre of mass.
    let reckoning = &skater.player_input.physical.reckoning;
    let body_position = reckoning.vector_64.map(f32::from_bits);
    // Snapshot +1F0 (packet +140) scaled by the graph at the previous
    // publication's packet +6C, |SystemReckoning +10|.
    let strengths = audio.player_physics.publish(
        skater
            .collision_feedback
            .audio
            .published
            .map(|contact| contact.intensity),
        player_physics::com_speed(reckoning.vector_16.map(f32::from_bits)),
        &bank.speed_graph,
    );
    if let Some(bands) = bank.grind_bands {
        match audio.grind.input(
            skater.player_input.grind_audio,
            bands,
            (0, false),
            &bank.materials,
        ) {
            Ok(Some(record)) => {
                info!(
                    "SKATE_AUDIO_GRIND_RECORD stamp={} materials={:?} categories={:?} levels={:?}",
                    audio.stamp, record.materials, record.categories, record.levels
                );
                audio.enqueue(record, body_position);
            }
            Ok(None) => {}
            Err(error) if !audio.failed => {
                audio.failed = true;
                warn!("Skating audio: grind start update rejected: {error:?}");
            }
            Err(_) => {}
        }
    }
    let published = &skater.collision_feedback.audio.published;
    let frame = body_impact::BodyFrame {
        // Regions 6 and 7 (feet) are scaled too; no ported consumer reads them.
        strengths: std::array::from_fn(|i| strengths[i]),
        surface_tags: std::array::from_fn(|i| published[i].material as i32),
        publication_ratio: mix::PUBLICATION_RATIO,
        state_2a4: skater.player_input.processed.flags_2468 & (1 << 18) != 0,
        state_2a5: skater.player_input.physical.skeleton.over_599 != 0,
        face_contact: skater.collision_feedback.specific[1].current,
        hall_of_meat: false,
        hall_of_meat_layers: [false; body_impact::REGIONS],
        channel_5: skate_core::audio::hom::duration::channel_5_strength(
            skater.player_input.physical.scoring.wipeout_duration_3518,
            skater
                .player_input
                .physical
                .collision
                .body_contact_strength_196,
        ),
        owner: (0, false),
    };
    let mut contacts_words = None;
    match audio.body.update(&frame, &bank.materials, &bank.settings) {
        Ok(update) => {
            contacts_words = Some(update.controller_words);
            for record in update.records {
                audio.enqueue(record, body_position);
            }
        }
        Err(error) if !audio.failed => {
            audio.failed = true;
            warn!("Skating audio: body impact update rejected: {error:?}");
        }
        Err(_) => {}
    }
    let deck = deck_impact::DeckFrame {
        strength: skater.player_input.deck_audio.published,
        surface_tag: skater.player_input.physical.collision.deck_audio_tag_12 as i32,
        off_board: skater.player_state.current().category() == 500,
        state_2a4: frame.state_2a4,
        publication_ratio: frame.publication_ratio,
        owner: frame.owner,
    };
    match audio
        .deck
        .update(&deck, &bank.materials, bank.board_cooldown)
    {
        Ok(records) => {
            // Snapshot +90: SkateboardReckoning +90, provisionally the
            // deck body position (layout evidence, not yet verified).
            for record in records {
                audio.enqueue(record, deck_position);
            }
        }
        Err(error) if !audio.failed => {
            audio.failed = true;
            warn!("Skating audio: deck impact update rejected: {error:?}");
        }
        Err(_) => {}
    }
    let BodyAudio { mix, groups, .. } = audio;
    if let Some(state) = mix.as_mut() {
        state.publish_player(contacts_words);
        state.publish_positions(groups.positions());
    }
}

fn playback(
    mut commands: Commands,
    clips: Res<Assets<AudioSource>>,
    mut collision_sources: ResMut<Assets<pan::CollisionSource>>,
    mut collision_output: ResMut<pan::CollisionOutput>,
    time: Res<Time<Real>>,
    samples: Option<Res<Samples>>,
    mods: Option<Res<crate::modding::Mods>>,
    mut mix: ResMut<Mix>,
    mut body_audio: ResMut<BodyAudio>,
    menu: Res<crate::menu::graphics_menu::Menu>,
    replay: Res<crate::replay::Replay>,
    transition: Res<crate::world::map_transition::MapTransition>,
    physics: Res<GamePhysics>,
    mut voices: Query<(Entity, Option<&mut AudioSink>), With<Voice>>,
    collision_voices: Query<
        (Entity, &pan::CollisionControl),
        (With<CollisionVoice>, Without<Voice>),
    >,
) {
    let Some(samples) = samples else {
        return;
    };
    let silence = menu.open
        || replay.active
        || transition.busy()
        || physics.failed
        || samples.volume == 0.
        || mods.as_deref().is_some_and(|m| {
            crate::modding::player_attached(m) || crate::modding::player_suspended(m)
        });
    if silence || mix.reset_voices {
        for (entity, sink) in &mut voices {
            if let Some(sink) = sink {
                sink.stop();
            }
            commands.entity(entity).despawn();
        }
        for (entity, control) in &collision_voices {
            collision_output.stop(control);
            commands.entity(entity).despawn();
        }
        collision_output.flush();
        // Host stops leave the original body-loop timers untouched.
        body_audio.stopping.clear();
        body_audio.reset_groups();
        if silence {
            *mix = default();
        } else {
            mix.loops = default();
            mix.pending.clear();
            mix.reset_voices = false;
        }
        return;
    }
    let desired = mix.tracker.desired;
    let speed = mix.tracker.speed;
    // Include pending spawns/despawns in the shared voice budget, since Commands
    // are deferred and the query will not reflect them until the next frame.
    let mut count = voices.iter().count();
    for (index, sound) in [Sound::Roll, Sound::Grind].into_iter().enumerate() {
        let selected = desired == Some(sound);
        if mix.loops[index].is_none() && selected && count < 8 {
            if let Some(clip) = samples.clips[sound.index()].first() {
                let entity = commands
                    .spawn((
                        Voice,
                        AudioPlayer::new(clip.clone()),
                        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.)),
                    ))
                    .id();
                mix.loops[index] = Some(LoopVoice {
                    entity,
                    envelope: default(),
                });
                count += 1;
            }
        }
        let Some(mut voice) = mix.loops[index] else {
            continue;
        };
        voice.envelope.update(selected, speed, time.delta_secs());
        if !selected && voice.envelope.fade == 0. {
            if let Ok((_, Some(sink))) = voices.get_mut(voice.entity) {
                sink.stop();
            }
            commands.entity(voice.entity).despawn();
            mix.loops[index] = None;
            count = count.saturating_sub(1);
            continue;
        }
        if let Ok((_, Some(mut sink))) = voices.get_mut(voice.entity) {
            let level = if sound == Sound::Roll { 0.35 } else { 0.7 };
            sink.set_volume(Volume::Linear(
                samples.volume * level * voice.envelope.gain * voice.envelope.fade,
            ));
            sink.set_speed(voice.envelope.pitch);
        }
        mix.loops[index] = Some(voice);
    }
    let events = std::mem::take(&mut mix.pending);
    for (sound, gain, contact) in events {
        if count >= 8 {
            break;
        }
        let mapped = (sound == Sound::Impact)
            .then(|| samples.materials.select(contact, mix.variant))
            .flatten();
        if contact.wheel.is_some() && mapped.is_none() {
            continue;
        }
        let (clip, material, trim, source) = if let Some(mapped) = mapped {
            mapped
        } else {
            let clips = &samples.clips[sound.index()];
            if clips.is_empty() {
                continue;
            }
            (&clips[mix.variant % clips.len()], "generic", 1., "generic")
        };
        if sound == Sound::Impact {
            info!(
                "SKATE_AUDIO_IMPACT audio_surface={} physics_surface={} body={:?} wheel={:?} strength={:.2} material={material} source={source} trim={trim:.3} gain={gain:.3}",
                contact.audio_surface,
                contact.physics_surface,
                contact.body,
                contact.wheel,
                contact.strength
            );
        }
        mix.variant = mix.variant.wrapping_add(1);
        commands.spawn((
            Voice,
            AudioPlayer::new(clip.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(samples.volume * 0.65 * gain * trim)),
        ));
        count += 1;
    }
    let Some(bank) = samples.collisions.as_ref() else {
        return;
    };
    if collision_output.needs_restart() {
        if let Some(entity) = collision_output.entity.take() {
            commands.entity(entity).try_despawn();
        }
        collision_output.reset_source();
    }
    if collision_output.entity.is_none() {
        let source = collision_sources.add(collision_output.asset());
        let entity = commands
            .spawn((AudioPlayer(source), PlaybackSettings::ONCE))
            .id();
        collision_output.entity = Some(entity);
    }
    for (entity, control) in &collision_voices {
        if control.finished() {
            commands.entity(entity).despawn();
        }
    }
    for voice in body_audio.stopping.drain(..) {
        if let Ok((_, control)) = collision_voices.get(voice) {
            collision_output.stop(control);
        }
        commands.entity(voice).try_despawn();
    }
    // Completion (824C0440..0468) resets the group and its mix state.
    for group in body_audio.groups.retain(|voice| {
        collision_voices
            .get(voice)
            .is_ok_and(|(_, control)| !control.finished())
    }) {
        if let Some(state) = body_audio.mix.as_mut() {
            state.reset_group(group);
        }
    }
    // Initial playback 824BFE08 follows the group's first evaluated phase.
    for (group, record) in body_audio.groups.unstarted() {
        let Some(&words) = body_audio.mix.as_ref().and_then(|state| state.words(group)) else {
            continue;
        };
        let mut sides = [None; 2];
        for (side, played) in sides.iter_mut().enumerate() {
            let Some(voice) = bank.side(&record, side, mix.variant) else {
                continue;
            };
            mix.variant = mix.variant.wrapping_add(1);
            let volume = voice.gain.volume(&words);
            let Some(clip) = clips.get(voice.clip) else {
                continue;
            };
            let Some(speed) = body_audio
                .mix
                .as_ref()
                .and_then(|state| state.speed(group, &voice.gain))
            else {
                continue;
            };
            let control = collision_output.start(clip, speed, &words, samples.volume * volume);
            let entity = commands.spawn((CollisionVoice, control)).id();
            body_audio.groups.attach(group, entity, voice.gain);
            *played = Some((voice.material, voice.bank, voice.event, volume));
        }
        body_audio.groups.start(group);
        let marker = if matches!(record.materials[0], 0x5f | 0x71) {
            "SKATE_AUDIO_DECK_RECORD"
        } else {
            "SKATE_AUDIO_BODY_RECORD"
        };
        info!(
            "{marker} group={group} materials={:?} categories={:?} levels={:?} flags={:?} volume_words={:04x?} voices={sides:?}",
            record.materials, record.categories, record.levels, record.flags, words
        );
    }
    // Each output phase recomputes playing voices' gain (824C0334..824C03DC).
    for (group, entity, gain) in body_audio.groups.voices() {
        let Some(words) = body_audio.mix.as_ref().and_then(|state| state.words(group)) else {
            continue;
        };
        if let Ok((_, control)) = collision_voices.get(entity) {
            if let Some(speed) = body_audio
                .mix
                .as_ref()
                .and_then(|state| state.speed(group, &gain))
            {
                collision_output.update(control, speed, words, samples.volume * gain.volume(words));
            }
        }
    }
    collision_output.flush();
}

/// Headless replay of the collision path for simulation tests: the same
/// manager phases as `sample`, with initial playback reduced to the gain each
/// side starts with. Voices count as finished after `hold` ticks; the game
/// uses clip completion instead.
#[cfg(test)]
pub(crate) struct CollisionProbe {
    audio: BodyAudio,
    bank: collisions::CollisionBank,
    world: World,
    voices: Vec<(Entity, u32)>,
    tick: u32,
    hold: u32,
}

/// One initial playback: the record, the group's camera distance and, per
/// audible side, (slot, live controller level, live gain, full-scale gain).
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ProbeStart {
    pub tick: u32,
    pub group: usize,
    pub record: body_impact::Record,
    pub distance: f32,
    pub sides: Vec<(usize, u16, f32, f32)>,
}

#[cfg(test)]
impl CollisionProbe {
    pub(crate) fn load(root: &std::path::Path, hold: u32) -> Option<Self> {
        let mut assets = Assets::<AudioSource>::default();
        let (bank, data) = collisions::CollisionBank::load(root, &mut assets)?;
        let mix = mix::MixState::new(data).ok()?;
        Some(Self {
            audio: BodyAudio {
                mix: Some(mix),
                ..default()
            },
            bank,
            world: World::new(),
            voices: Vec::new(),
            tick: 0,
            hold,
        })
    }

    pub(crate) fn tick(
        &mut self,
        physics: &GamePhysics,
        skater: &SkaterRuntime,
        camera: &crate::camera::CameraRuntime,
        delta: f32,
    ) -> Vec<ProbeStart> {
        self.tick += 1;
        let listener = camera.presentation_frame().map(|frame| {
            let at = frame.basis.columns[2];
            (frame.position, [at[0], at[1], at[2], 0.0])
        });
        let deck = physics.board.bodies()[skate_core::physics::board::BodyId::Deck.index()]
            .rates
            .position;
        update_body(
            &mut self.audio,
            &self.bank,
            skater,
            delta,
            listener,
            [deck.x, deck.y, deck.z, 0.0],
        );
        for voice in self.audio.stopping.drain(..) {
            self.world.despawn(voice);
        }
        let (tick, hold) = (self.tick, self.hold);
        let world = &mut self.world;
        self.voices.retain(|&(voice, start)| {
            let playing = world.get_entity(voice).is_ok() && tick - start < hold;
            if !playing {
                let _ = world.try_despawn(voice);
            }
            playing
        });
        for group in self
            .audio
            .groups
            .retain(|voice| self.world.get_entity(voice).is_ok())
        {
            self.audio.mix.as_mut().unwrap().reset_group(group);
        }
        let mut starts = Vec::new();
        for (group, record) in self.audio.groups.unstarted() {
            let state = self.audio.mix.as_ref().expect("live collision mix");
            let words = *state.words(group).unwrap();
            let distance = f32::from_bits(state.position_words(group).unwrap()[1]);
            let mut sides = Vec::new();
            for side in 0..2 {
                let Some(voice) = self.bank.side(&record, side, 0) else {
                    continue;
                };
                let level =
                    skate_core::audio::voice::controller_level(Some(&words), voice.gain.slot)
                        .unwrap();
                sides.push((
                    voice.gain.slot,
                    level,
                    voice.gain.volume(&words),
                    voice.gain.volume(&collisions::FULL_SCALE),
                ));
                let entity = self.world.spawn_empty().id();
                self.voices.push((entity, self.tick));
                self.audio.groups.attach(group, entity, voice.gain);
            }
            self.audio.groups.start(group);
            starts.push(ProbeStart {
                tick: self.tick,
                group,
                record,
                distance,
                sides,
            });
        }
        starts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(state: State, x: f32) -> Frame {
        Frame {
            state,
            position: Vec3::new(x, 0., 0.),
            speed: 4.,
            generation: 0,
            discontinuity: false,
            trick_sequence: 0,
            contacts: default(),
            wheels: None,
            wheel_surface: 0,
            dt: 1. / 60.,
        }
    }
    #[test]
    fn wheel_landing_and_separate_deck_collision_remain_independent() {
        let mut tracker = Tracker::default();
        let mut current = frame(State::PhysicsAir, 0.);
        current.wheels = Some(wheels::Input {
            touching: [false; 4],
            air_time: 0.1,
            airborne: true,
            mode: 0,
            thresholds: [0.31, 0.5],
        });
        tracker.update(current);
        current.state = State::PhysicsGround;
        current.wheels.as_mut().unwrap().touching = [true; 4];
        current.wheels.as_mut().unwrap().airborne = false;
        tracker.update(current);
        assert_eq!(tracker.impact_events.len(), 1);
        assert!(tracker.impact_events[0].1.wheel.is_some());
        // A truck/deck wall hit while wheels remain grounded must still sound.
        current.contacts[0].strength = 120.;
        tracker.update(current);
        assert_eq!(tracker.impact_events.len(), 1);
        assert!(tracker.impact_events[0].1.wheel.is_none());
        current.discontinuity = true;
        tracker.update(current);
        assert!(tracker.impact_events.is_empty());
        current.discontinuity = false;
        current.contacts = default();
        tracker.update(current);
        assert!(tracker.impact_events.is_empty());
    }
    #[test]
    fn actual_contacts_sound_even_during_bails_without_a_state_change() {
        for state in [
            State::PhysicsGround,
            State::WipeoutGround,
            State::BipedGround,
        ] {
            let mut tracker = Tracker::default();
            let mut current = frame(state, 0.);
            tracker.update(current);
            current.contacts[0].strength = 120.;
            assert_eq!(tracker.update(current), vec![Sound::Impact]);
            for _ in 0..20 {
                assert!(tracker.update(current).is_empty());
            }
        }
    }
    #[test]
    fn board_torso_and_head_hits_do_not_share_a_debounce_latch() {
        let mut tracker = Tracker::default();
        let mut current = frame(State::WipeoutGround, 0.);
        tracker.update(current);
        current.contacts[0].strength = 80.;
        assert_eq!(tracker.update(current), vec![Sound::Impact]);
        let torso = impact::Contact {
            strength: 40.,
            body: Some(impact::BodyPart::Torso),
            ..default()
        };
        current.contacts[torso.channel()] = torso;
        assert_eq!(tracker.update(current), vec![Sound::Impact]);
        assert_eq!(tracker.impact_events[0].1.body, torso.body);
        let head = impact::Contact {
            strength: 100.,
            body: Some(impact::BodyPart::Head),
            ..default()
        };
        current.contacts[head.channel()] = head;
        assert_eq!(tracker.update(current), vec![Sound::Impact]);
        assert_eq!(tracker.impact_events[0].1.body, head.body);
        assert!(tracker.update(current).is_empty());
    }

    #[test]
    fn crowded_bails_keep_only_the_two_strongest_new_impact_families() {
        let mut tracker = Tracker::default();
        let mut current = frame(State::WipeoutGround, 0.);
        tracker.update(current);
        for (body, strength) in [
            (None, 40.),
            (Some(impact::BodyPart::Head), 100.),
            (Some(impact::BodyPart::Torso), 200.),
            (Some(impact::BodyPart::Arm), 80.),
        ] {
            let contact = impact::Contact {
                body,
                strength,
                ..default()
            };
            current.contacts[contact.channel()] = contact;
        }
        assert_eq!(tracker.update(current), vec![Sound::Impact, Sound::Impact]);
        assert_eq!(
            tracker
                .impact_events
                .iter()
                .map(|hit| hit.1.strength)
                .collect::<Vec<_>>(),
            vec![200., 100.]
        );
        assert!(tracker.update(current).is_empty());
    }

    #[test]
    fn falling_or_changing_state_without_a_contact_is_silent() {
        let mut tracker = Tracker::default();
        tracker.update(frame(State::KnownAir, 0.));
        for _ in 0..10 {
            assert!(tracker.update(frame(State::KnownAir, 0.)).is_empty());
        }
        assert!(tracker.update(frame(State::WipeoutGround, 0.)).is_empty());
        assert!(tracker.update(frame(State::PhysicsGround, 0.)).is_empty());
    }
    #[test]
    fn teleport_clears_pending_impact_history() {
        let mut tracker = Tracker::default();
        tracker.update(frame(State::KnownAir, 0.));
        let mut next = frame(State::PhysicsGround, 100.);
        next.discontinuity = true;
        next.contacts[0].strength = 120.;
        assert!(tracker.update(next).is_empty());
        assert!(tracker.reset);
    }
    #[test]
    fn vertical_bobbing_does_not_trigger_footsteps() {
        let mut tracker = Tracker::default();
        tracker.update(frame(State::BipedGround, 0.));
        for i in 0..120 {
            let mut step = frame(State::BipedGround, 0.);
            step.position.y = if i % 2 == 0 { 0.1 } else { 0. };
            assert!(tracker.update(step).is_empty());
        }
    }
    #[test]
    fn loops_follow_ground_grind_air_and_bail() {
        let mut tracker = Tracker::default();
        tracker.update(frame(State::PhysicsGround, 0.));
        for (state, expected) in [
            (State::PhysicsGround, Some(Sound::Roll)),
            (State::GrindFiftyFifty, Some(Sound::Grind)),
            (State::KnownAir, None),
            (State::WipeoutGround, None),
        ] {
            tracker.update(frame(state, 0.1));
            assert_eq!(tracker.desired, expected);
        }
    }
    #[test]
    fn transitions_do_not_emit_teleport_or_load_sounds() {
        let mut tracker = Tracker::default();
        tracker.update(frame(State::PhysicsGround, 0.));
        let mut next = frame(State::SlideGround, 0.1);
        next.discontinuity = true;
        assert!(tracker.update(next).is_empty());
        assert_eq!(tracker.desired, None);
        next = frame(State::BipedGround, 100.);
        next.generation = 1;
        assert!(tracker.update(next).is_empty());
        tracker = default(); // pause/replay clears all history
        assert!(tracker.update(frame(State::SlideGround, 0.)).is_empty());
    }
    #[test]
    fn footsteps_need_distance_and_each_trick_sequence_plays_once() {
        let mut tracker = Tracker::default();
        tracker.update(frame(State::BipedGround, 0.));
        assert!(tracker.update(frame(State::BipedGround, 0.)).is_empty());
        assert_eq!(
            tracker.update(frame(State::BipedGround, 1.)),
            vec![Sound::Step]
        );
        let mut air = frame(State::KnownAir, 1.);
        air.trick_sequence = 1;
        assert_eq!(tracker.update(air), vec![Sound::Flip]);
        assert!(tracker.update(air).is_empty());
        air.trick_sequence = 2;
        assert_eq!(tracker.update(air), vec![Sound::Flip]);
    }
}
