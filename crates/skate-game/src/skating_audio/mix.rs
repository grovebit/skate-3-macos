//! Live MixMap volume and pitch controls for the original collision voices.
//!
//! Producers publish in the audio manager's input phase, in original family
//! order: listener (8247AA48), Main objects, the local player's Contacts and
//! PlayerPhysics, then each active collision group's 3DColPos. MixMap phases
//! evaluate the collision volume/pitch cone (`skate_core::audio::collision_mix`);
//! voices read their group's packed words afterwards.
//!
//! Host state supplied here: the port has no front-end audio states, NIS,
//! movies, challenges, speech or scripted fades, so those manager fields keep
//! their constructor values and the related producers publish what the
//! original publishes in ordinary free skate. Pause, replay and map loads stop
//! collision playback in the host instead of running the Pause duck envelope.
use super::collisions::{MixData, MixSettings};
use skate_core::audio::{
    collision_mix::{CollisionMix, MixError},
    collision_position::{self, Listener},
    curve::CurveTable,
    hom,
    input::WordInputs,
    main_objects::{self, ManagerState, PauseQueries, PauseState},
    master::{ProfileVolumes, PublicationSpeed, TransitionTimer, state_slot0},
    scalar::ScalarTables,
};

// Canonical owners of the Main-family objects (registration 82FD1310..),
// the local player's Player-family instance 0 and the collision positions.
const MASTER: u32 = 0x4000_0020;
const CAMERAMAN: u32 = 0x4000_0030;
const NIS: u32 = 0x4000_0060;
const PAUSE: u32 = 0x4000_0070;
const SPEECH: u32 = 0x4000_0080;
const BLOOM: u32 = 0x4000_0090;
const VU: u32 = 0x4000_00a0;
const CHALLENGE: u32 = 0x4000_00b0;
const HOM: u32 = 0x4000_00c0;
const MENU: u32 = 0x4000_00d0;
const CONTACTS: u32 = 0x4001_0010;
const PLAYER_PHYSICS: u32 = 0x6001_0000;
const COLLISION_POSITION: u32 = 0x6003_0000;

/// Publication header +00 (context +29070), retained at its constructed
/// ratio 1 like the body loop's snapshot +DC; its later writers are untraced.
pub(super) const PUBLICATION_RATIO: f32 = 1.0;

fn position_owner(group: usize) -> u32 {
    COLLISION_POSITION | ((group as u32) << 11)
}

pub(super) struct MixState {
    mix: CollisionMix,
    curves: CurveTable,
    tables: ScalarTables,
    settings: MixSettings,
    pitch: skate_core::audio::pitch::PitchTables,
    inputs: WordInputs,
    listener: Option<Listener>,
    master: TransitionTimer,
    pause: PauseState,
    hom: hom::State,
    challenge_latch: u8,
}

impl MixState {
    pub fn new(data: MixData) -> Result<Self, MixError> {
        let families = data
            .program
            .get(4..8)
            .map(|w| u32::from_be_bytes(w.try_into().unwrap()) as usize)
            .filter(|n| (4..=256).contains(n))
            .ok_or(MixError::Program)?;
        // Ordinary initialization: Main 1 group (824DF8A0), Player 2
        // (824DFA98), Collision 10 (824DF3A0). Other families are not in
        // the collision cone; a reference into them fails to bind.
        let mut counts = vec![0; families];
        counts[0] = 1;
        counts[1] = 2;
        counts[3] = 10;
        let mix = CollisionMix::from_mxb(&data.program, &counts, &data.tables)?;
        let mut inputs = WordInputs::default();
        // The evaluator creates zeroed input buffers for every reference;
        // the second Player instance stays inactive and never publishes.
        for owner in mix.required_inputs() {
            inputs.attach(owner).ok_or(MixError::Unsupported(owner))?;
        }
        Ok(Self {
            mix,
            curves: data.curves,
            tables: data.tables,
            settings: data.settings,
            pitch: data.pitch,
            inputs,
            listener: None,
            master: TransitionTimer::default(),
            pause: PauseState::default(),
            hom: hom::State::default(),
            challenge_latch: 0,
        })
    }

    fn owner_words(&mut self, owner: u32) -> &mut [u32; 16] {
        self.inputs.attach(owner).expect("canonical owner")
    }

    /// Listener update 8247AA48 and the Main family's input updates.
    /// `camera` is the rendered camera's position and "at" row; without one
    /// the previous listener is retained, as the original root is.
    pub fn publish_main(&mut self, elapsed: f32, camera: Option<([f32; 4], [f32; 4])>) {
        if let Some((position, at)) = camera {
            self.listener = Some(Listener::from_camera(position, at));
        }
        let manager = ManagerState::default();
        let ratio = PUBLICATION_RATIO;
        // Manager +34B/+34C need publication bits 26/25 from the actor's
        // +CC object, whose writer is untraced. They stay clear; at a ratio
        // of 1 the HOM slots and the Master treatment do not depend on them.
        let hom_active = false;
        let settings = self.settings;
        let mut timer = self.master;
        {
            let master = self.owner_words(MASTER);
            // Context present; both ordinary providers return zero (82A248A0).
            master[0] = state_slot0(Some(false));
            ProfileVolumes::default()
                .publication()
                .expect("constructed profile volumes are in range")
                .write(Some(master));
            let speed = PublicationSpeed::speed(Some(ratio), hom_active, settings.treatment_scale)
                .expect("finite ratio");
            PublicationSpeed::evaluate(Some(ratio), hom_active, settings.treatment_scale)
                .expect("finite ratio")
                .write(master);
            timer.publish(manager.fe_state, elapsed, Some(false), speed, master);
        }
        self.master = timer;
        let mut hom_state = self.hom;
        let hom_words = self.owner_words(HOM);
        hom_state.publish(
            &hom::Inputs {
                manager_34b: u8::from(hom_active),
                manager_34c: 0,
                timing_ratio: ratio,
                secondary_flags: None,
                manager_49c: manager.fe_state,
                manager_36c: manager.event_type,
                manager_1cc: manager.nis_flag,
            },
            true,
            |slot, value| hom_words[slot] = value,
        );
        self.hom = hom_state;
        // Collision playback runs only while the host is unpaused.
        let mut pause = self.pause;
        pause.publish(
            &manager,
            &PauseQueries {
                paused: false,
                fe_query: false,
                master_query: false,
                movie_core: false,
            },
            self.owner_words(PAUSE),
        );
        self.pause = pause;
        let fe_query = main_objects::fe_query(&manager, false);
        main_objects::publish_nis(&manager, fe_query, self.owner_words(NIS));
        main_objects::publish_menu_states(&manager, self.owner_words(MENU));
        let (slot5, latch) = main_objects::challenge_slot5(self.challenge_latch, Some(ratio));
        self.challenge_latch = latch;
        self.owner_words(CHALLENGE)[5] = slot5;
        // No speech system: the speech manager has no records or entries.
        self.owner_words(CAMERAMAN)[0] = main_objects::cameraman_slot0(None);
        self.owner_words(SPEECH)[1] = main_objects::speech_slot1(&[None, None]);
        // No visual transition (manager +4A8 stays 0.0) or scripted fade.
        self.owner_words(BLOOM)[0] = main_objects::bloom_slot0(&manager, 0.0, None);
        // VU slot 1 meters an audio-core bus the host mixer does not expose.
        // It only weights the CameraMan envelope, idle without speech.
        self.owner_words(VU)[1] = 0;
    }

    /// The local player's Player-family publications: Contacts words 7/8
    /// (824AA020) and PlayerPhysics slot 9, zero for the selected local
    /// player (8249FC7C..8249FCBC, group +48 = packet +98 bit 31).
    pub fn publish_player(&mut self, contacts: Option<[u16; 2]>) {
        if let Some([word7, word8]) = contacts {
            let words = self.owner_words(CONTACTS);
            words[7] = u32::from(word7);
            words[8] = u32::from(word8);
        }
        self.owner_words(PLAYER_PHYSICS)[9] = 0;
    }

    /// Group reset 828B7E60: output word +3C cleared and the linked input
    /// buffers zeroed (position reset 8249CAC8 clears the source).
    pub fn reset_group(&mut self, group: usize) {
        self.mix.reset(group);
        self.inputs.clear(position_owner(group));
    }

    /// Voice activation 824BFC58 enables the group's output.
    pub fn activate_group(&mut self, group: usize) {
        self.mix.activate(group);
    }

    /// Collision family input phase: only active groups publish (828B7C58).
    pub fn publish_positions(&mut self, positions: impl Iterator<Item = (usize, [f32; 4])>) {
        let Some(listener) = self.listener else {
            return;
        };
        let offset = self.settings.listener_offset;
        for (group, position) in positions {
            let words = self.owner_words(position_owner(group));
            collision_position::publish(words, Some(position), &listener, offset);
        }
    }

    pub fn advance(&mut self, elapsed: f32) -> Result<(), MixError> {
        self.mix
            .advance(elapsed, &mut self.inputs, &self.curves, &self.tables)
    }

    pub fn words(&self, group: usize) -> Option<&[u32; 16]> {
        self.mix.words(group)
    }

    /// Ordinary voice consumer 824C0158 / 824C03D4..824C0408.
    pub fn speed(&self, group: usize, gain: &super::collisions::VoiceGain) -> Option<f32> {
        skate_core::audio::pitch::collision_pitch(
            gain.authored_pitch,
            self.words(group).map(|w| w.as_slice()),
            gain.pitch_slot,
            &self.pitch,
        )
        .ok()
        .map(|pitch| pitch.ratio)
    }

    /// The group's published 3DColPos words (word 1: camera distance).
    #[cfg(test)]
    pub fn position_words(&self, group: usize) -> Option<&[u32; 16]> {
        self.inputs.words(position_owner(group))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skate_core::audio::clock::AudioClock;

    /// Drives the adapter through manager phases with a camera at the origin
    /// looking down +Z and one active group at `distance` straight ahead.
    fn settle(state: &mut MixState, distance: f32, ticks: usize) {
        let mut clock = AudioClock::default();
        for _ in 0..ticks {
            let phases = clock.advance(1. / 60., true, false);
            if let Some(elapsed) = phases.inputs {
                state.publish_main(elapsed, Some(([0., 1.6, 0., 1.], [0., 0., 1., 0.])));
                state.publish_player(Some([0, 0]));
                state.publish_positions([(0, [0., 1.6, distance, 1.])].into_iter());
            }
            if let Some(elapsed) = phases.mixmap {
                state.advance(elapsed).unwrap();
            }
        }
    }

    #[test]
    #[ignore = "requires the owned collision bank and updated mixmap/ export"]
    fn owned_body_deck_and_grind_use_live_pitch_words() {
        use super::super::collisions::CollisionBank;
        use skate_core::audio::body_impact::{ABSENT, Record};
        let root = std::env::var_os("SKATE_OWNED_AUDIO_ROOT").expect("owned audio library");
        let mut assets = bevy::prelude::Assets::<bevy::audio::AudioSource>::default();
        let (bank, data) = CollisionBank::load(std::path::Path::new(&root), &mut assets).unwrap();
        let mut state = MixState::new(data).unwrap();
        state.activate_group(0);
        settle(&mut state, 5.0, 60);
        let words = state.words(0).unwrap();
        assert_eq!(((words[0] >> 16) as i16, words[11] as i16), (-4, 0));
        // Torso, deck and both grind-start material families (5F/60).
        for (material, combined) in [(0x62, 3786), (0x5f, 4086), (0x60, 3088)] {
            let record = Record {
                materials: [material, ABSENT],
                categories: [0, 3],
                levels: [32767, 0],
                flags: [0; 3],
            };
            let voice = bank.side(&record, 0, 0).unwrap();
            assert_eq!(voice.gain.pitch_slot, 1);
            assert_eq!(state.speed(0, &voice.gain), Some(combined as f32 / 4096.0));
        }
    }

    #[test]
    #[ignore = "requires the owned mixmap/ export"]
    fn owned_pan_tracks_camera_rotation_in_the_existing_volume_cone() {
        let root = std::env::var_os("SKATE_OWNED_AUDIO_ROOT").expect("owned audio library");
        let root = std::path::Path::new(&root).join("material-impacts");
        let mut state =
            MixState::new(super::super::collisions::MixData::load(&root).unwrap()).unwrap();
        state.activate_group(0);
        let source = [5., 1.6, 0., 1.]; // queued position stays fixed
        for at in [[0., 0., 1., 0.], [1., 0., 0., 0.], [0., 0., -1., 0.]] {
            state.publish_main(1. / 30., Some(([0., 1.6, 0., 1.], at)));
            state.publish_player(Some([0, 0]));
            state.publish_positions([(0, source)].into_iter());
            let phase = state.position_words(0).unwrap()[3];
            state.advance(1. / 30.).unwrap();
            assert_eq!(state.words(0).unwrap()[0] & 0xffff, phase);
            let gains = skate_core::audio::pan::mono_stereo(
                skate_core::audio::pan::collision_degrees(state.words(0).map(|w| w.as_slice()))
                    .unwrap(),
            );
            if at[0] == 1. {
                assert!((gains[0] - gains[1]).abs() < 1e-6);
            } else if at[2] > 0. {
                assert!(phase > 32767 && gains[0] > gains[1]);
            } else {
                assert!(phase < 32767 && gains[1] > gains[0]);
            }
        }
    }

    #[test]
    #[ignore = "requires the owned mixmap/ export"]
    fn owned_body_volume_follows_camera_distance_after_the_master_startup_pulse() {
        let root = std::env::var_os("SKATE_OWNED_AUDIO_ROOT")
            .expect("set SKATE_OWNED_AUDIO_ROOT to the audio library");
        let root = std::path::Path::new(&root).join("material-impacts");
        let data = super::super::collisions::MixData::load(&root).expect("mixmap/");
        assert_eq!(data.settings.listener_offset, 0.25);
        let mut state = MixState::new(data).unwrap();
        state.activate_group(0);
        let slot18 = |state: &MixState| state.words(0).unwrap()[9] & 0xffff;
        // Master slot 6 publishes once at construction (+40 = 0.0), so the
        // -10000 envelope F0.1 ducks collisions at startup, then releases.
        settle(&mut state, 5., 2);
        let startup = slot18(&state);
        settle(&mut state, 5., 60);
        assert!(startup < slot18(&state), "{startup}");
        // Same value as the independent reference checkers at 5 m, phase 0.
        assert_eq!(slot18(&state), 7053);
        settle(&mut state, 12., 4);
        assert_eq!(slot18(&state), 3984);
        // A reset group publishes nothing and its output is disabled.
        state.reset_group(0);
        settle(&mut state, 12., 4);
        assert_eq!(slot18(&state), 3984);
        state.activate_group(0);
        settle(&mut state, 1., 4);
        assert_eq!(slot18(&state), 9202);
    }
}
