//! Read-only observations of completed external contacts. No gameplay state
//! transition is needed: the same observer works while riding and ragdolling.
use crate::physics::{GamePhysics, SkaterRuntime};
use skate_core::physics::board_step::CollisionBody;
use skate_core::physics::skeleton_body::SkeletonCollisionFeedback;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum BodyPart {
    Head,
    Torso,
    Arm,
    Leg,
    Foot,
}

impl BodyPart {
    /// Use the recovered physics regions, including torso parts6 and10.
    /// The provisional audio channels still merge left/right limbs.
    fn from_skeleton(part: usize) -> Option<Self> {
        match SkeletonCollisionFeedback::contact_region(part)? {
            0 => Some(Self::Head),
            1 => Some(Self::Torso),
            2 | 3 => Some(Self::Arm),
            4 | 5 => Some(Self::Leg),
            6 | 7 => Some(Self::Foot),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Contact {
    /// Solver position impulse / dt, in momentum units. Resting support stays
    /// below the onset threshold; impact thresholds are provisional host tuning.
    pub strength: f32,
    pub audio_surface: u8,
    pub physics_surface: u8,
    pub body: Option<BodyPart>,
    pub wheel: Option<super::wheels::Event>,
}

impl Contact {
    pub fn channel(self) -> usize {
        self.body.map_or(0, |part| part as usize + 1)
    }

    fn threshold(self) -> f32 {
        match self.body {
            Some(BodyPart::Foot) => 32.,
            Some(_) => 20.,
            None => 12.,
        }
    }
}

pub(super) type Contacts = [Contact; 6];

pub(super) fn trace_body_audio(
    physics: &GamePhysics,
    skater: &SkaterRuntime,
    contacts: &Contacts,
    materials: Option<&super::materials::Materials>,
) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("SKATE_AUDIO_BODY_TRACE").is_some()) {
        return;
    }
    // The physics publication (packet +140) before the PlayerPhysics speed
    // graph; `body_layers` classifies these unscaled strengths. `com_speed`
    // is the packet +6C the graph reads one audio publication later.
    let audio = &skater.collision_feedback.audio;
    let body_layers = materials
        .and_then(|bank| bank.body_levels(std::array::from_fn(|i| audio.published[i].intensity)));
    let body_events = materials.zip(body_layers).and_then(|(bank, layers)| {
        bank.body_event_layers(layers, std::array::from_fn(|i| audio.published[i].material))
    });
    let velocity = skater.player_input.physical.reckoning.vector_16;
    bevy::log::info!(
        body_event_candidates = ?body_events,
        requested_simulation_timestep = physics.requested_simulation_timestep(),
        body_layers = ?body_layers,
        state = ?skater.player_state.current(),
        current = ?audio.current.map(|contact| contact.intensity),
        published = ?audio.published.map(|contact| contact.intensity),
        materials = ?audio.published.map(|contact| contact.material),
        com_speed = skate_core::audio::player_physics::com_speed(velocity.map(f32::from_bits)),
        host_impulses = ?contacts.map(|contact| contact.strength),
        "SKATE_AUDIO_BODY_INPUT"
    );
}

pub(super) fn contacts(
    physics: &GamePhysics,
    skater: &SkaterRuntime,
    wheel_audio: bool,
    deck_audio: bool,
) -> (Contacts, Option<u8>) {
    let dt = physics.settings.step.simulation.time_step;
    if !dt.is_finite() || dt <= 0. {
        return Default::default();
    }
    let remote_base = skater.skeleton.bodies().len() + skater.skeleton_drives.targets.bodies.len();
    let mut best = Contacts::default();
    let mut wheel_surface = None;
    for row in physics.board.solved_contacts() {
        let words = row.words();
        // Match the original post-solver spy's enabled/positive-normal filter.
        if words[11] & 8 == 0 {
            continue;
        }
        let a = CollisionBody::from_contact_id(words[31]);
        let b = CollisionBody::from_contact_id(words[43]);
        let Some((side_a, body)) = owner(a, b, remote_base) else {
            continue;
        };
        let strength = row.accumulated_impulse()[0] / dt;
        if !strength.is_finite() || strength <= 0. {
            continue;
        }
        let tag = if side_a {
            words[55] as u16
        } else {
            (words[55] >> 16) as u16
        };
        let board_part = if side_a { a } else { b };
        if let CollisionBody::Board(part) = board_part {
            if part.index() == 0 {
                wheel_surface = Some((tag & 0x7f) as u8);
            }
            if wheel_audio && part.index() < 4 {
                continue;
            }
            if deck_audio && part == skate_core::physics::board::BodyId::Deck {
                continue;
            }
        }
        let candidate = Contact {
            strength,
            audio_surface: (tag & 0x7f) as u8,
            physics_surface: ((tag >> 7) & 0x1f) as u8,
            body,
            wheel: None,
        };
        retain_contact(&mut best, candidate);
    }
    (best, wheel_surface)
}

fn retain_contact(contacts: &mut Contacts, candidate: Contact) {
    let best = &mut contacts[candidate.channel()];
    if candidate.strength > best.strength {
        *best = candidate;
    }
}

fn owner(
    a: CollisionBody,
    b: CollisionBody,
    remote_base: usize,
) -> Option<(bool, Option<BodyPart>)> {
    let local = |id| match id {
        CollisionBody::Board(_) => Some(None),
        CollisionBody::Attached(part) if part < 24 => Some(BodyPart::from_skeleton(part)),
        _ => None,
    };
    let external = |id| {
        matches!(id, CollisionBody::StaticWorld)
            || matches!(id, CollisionBody::Attached(part) if part >= remote_base)
    };
    if external(b) {
        local(a).map(|body| (true, body))
    } else if external(a) {
        local(b).map(|body| (false, body))
    } else {
        None
    }
}

/// Kept outside the mix so muting/resetting voices cannot replay old contacts.
#[derive(Default)]
pub(super) struct FreshContacts(Option<(u64, u64)>);
impl FreshContacts {
    pub fn observe(&mut self, map: u64, solve: u64) -> bool {
        let current = (map, solve);
        let fresh = solve != 0 && self.0 != Some(current);
        self.0 = Some(current);
        fresh
    }
}

#[derive(Default)]
pub(super) struct Onset {
    cooldown: f32,
    latched: bool,
    last_hit: f32,
}
impl Onset {
    pub fn update(&mut self, contact: Contact, dt: f32) -> Option<f32> {
        self.cooldown = (self.cooldown - dt.clamp(0., 0.1)).max(0.);
        let strength = if contact.strength.is_finite() {
            contact.strength.max(0.)
        } else {
            0.
        };
        // Feet support the skater and can pulse while standing/walking. These
        // host thresholds separate that support from a distinct collision.
        let threshold = contact.threshold();
        if strength < threshold * 0.5 {
            self.latched = false;
        }
        let new_hit = strength >= threshold && (!self.latched || strength > self.last_hit * 2.);
        if !new_hit {
            return None;
        }
        self.latched = true;
        // A light touch must not mask a hard landing inside the debounce window.
        if self.cooldown > 0. && strength <= self.last_hit * 2. {
            return None;
        }
        self.cooldown = 0.12;
        self.last_hit = strength;
        Some(((strength - threshold) / 100.).sqrt().clamp(0.08, 1.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skate_core::physics::board::BodyId;

    #[test]
    fn contacts_are_consumed_once_per_solve_including_map_changes() {
        let mut fresh = FreshContacts::default();
        assert!(!fresh.observe(0, 0));
        assert!(fresh.observe(0, 1));
        assert!(!fresh.observe(0, 1));
        assert!(fresh.observe(0, 2));
        assert!(!fresh.observe(1, 0));
        assert!(fresh.observe(1, 1));
        assert!(fresh.observe(2, 1));
    }

    #[test]
    fn physical_parts_select_head_limbs_and_feet_without_including_board_root() {
        assert_eq!(BodyPart::from_skeleton(0), None);
        assert_eq!(BodyPart::from_skeleton(1), Some(BodyPart::Head));
        for part in [3, 4, 5, 7, 8, 9] {
            assert_eq!(BodyPart::from_skeleton(part), Some(BodyPart::Arm));
        }
        for part in [15, 16, 19, 20] {
            assert_eq!(BodyPart::from_skeleton(part), Some(BodyPart::Foot));
        }
        for part in [17, 18, 21, 22] {
            assert_eq!(BodyPart::from_skeleton(part), Some(BodyPart::Leg));
        }
        for part in [2, 6, 10, 11, 12, 13, 14, 23] {
            assert_eq!(BodyPart::from_skeleton(part), Some(BodyPart::Torso));
        }
        assert_eq!(BodyPart::from_skeleton(24), None);
    }

    #[test]
    fn external_board_and_body_contacts_are_selected_in_either_order() {
        let deck = CollisionBody::Board(BodyId::Deck);
        let body = CollisionBody::Attached(3);
        let world = CollisionBody::StaticWorld;
        assert_eq!(owner(deck, world, 50), Some((true, None)));
        assert_eq!(owner(world, body, 50), Some((false, Some(BodyPart::Arm))));
        assert_eq!(owner(body, deck, 50), None);
        assert_eq!(owner(body, CollisionBody::Attached(30), 50), None);
        assert_eq!(
            owner(body, CollisionBody::Attached(50), 50),
            Some((true, Some(BodyPart::Arm)))
        );
        assert_eq!(owner(world, CollisionBody::Attached(60), 50), None);
    }
    #[test]
    fn resting_contacts_are_silent_and_persistent_impacts_do_not_repeat() {
        let mut onset = Onset::default();
        let contact = |strength| Contact {
            strength,
            ..Default::default()
        };
        for _ in 0..120 {
            assert_eq!(onset.update(contact(4.), 1. / 60.), None);
        }
        assert!(onset.update(contact(80.), 1. / 60.).is_some());
        for _ in 0..120 {
            assert_eq!(onset.update(contact(80.), 1. / 60.), None);
        }
        onset.update(contact(0.), 1. / 60.);
        assert!(onset.update(contact(80.), 1. / 60.).is_some());
    }
    #[test]
    fn quiet_foot_support_cannot_mask_a_valid_board_impact() {
        let foot = Contact {
            strength: 20.,
            body: Some(BodyPart::Foot),
            ..Default::default()
        };
        let board = Contact {
            strength: 18.,
            ..Default::default()
        };
        for order in [[foot, board], [board, foot]] {
            let mut contacts = Contacts::default();
            for contact in order {
                retain_contact(&mut contacts, contact);
            }
            assert!(
                Onset::default()
                    .update(contacts[board.channel()], 1. / 60.)
                    .is_some()
            );
            assert!(
                Onset::default()
                    .update(contacts[foot.channel()], 1. / 60.)
                    .is_none()
            );
        }
    }

    #[test]
    fn a_hit_that_builds_over_several_ticks_can_get_louder() {
        let mut onset = Onset::default();
        let contact = |strength| Contact {
            strength,
            ..Default::default()
        };
        let first = onset.update(contact(20.), 1. / 60.).unwrap();
        assert_eq!(onset.update(contact(30.), 1. / 60.), None);
        assert!(onset.update(contact(45.), 1. / 60.).unwrap() > first);
    }

    #[test]
    fn threshold_chatter_does_not_repeat_until_contact_releases() {
        let mut onset = Onset::default();
        let contact = |strength| Contact {
            strength,
            ..Default::default()
        };
        assert!(onset.update(contact(13.), 1. / 60.).is_some());
        for _ in 0..120 {
            assert_eq!(onset.update(contact(11.), 1. / 60.), None);
            assert_eq!(onset.update(contact(13.), 1. / 60.), None);
        }
        onset.update(contact(0.), 1. / 60.);
        assert!(onset.update(contact(13.), 1. / 60.).is_some());
    }

    #[test]
    fn foot_support_is_quiet_but_a_hard_foot_landing_sounds() {
        let mut onset = Onset::default();
        let contact = |strength| Contact {
            strength,
            body: Some(BodyPart::Foot),
            ..Default::default()
        };
        for strength in [0., 12.92, 14.73, 13.20, 0., 20.] {
            assert_eq!(onset.update(contact(strength), 1. / 60.), None);
        }
        assert!(onset.update(contact(150.), 1. / 60.).is_some());
    }

    #[test]
    fn a_small_touch_does_not_mask_a_harder_hit_during_cooldown() {
        let mut onset = Onset::default();
        let contact = |strength| Contact {
            strength,
            ..Default::default()
        };
        let small = onset.update(contact(20.), 1. / 60.).unwrap();
        let large = onset.update(contact(200.), 1. / 60.).unwrap();
        assert!(large > small);
        assert_eq!(onset.update(contact(210.), 1. / 60.), None);
    }

    #[test]
    fn harder_impacts_are_louder_and_nearby_contact_rows_are_debounced() {
        let contact = |strength| Contact {
            strength,
            ..Default::default()
        };
        let soft = Onset::default().update(contact(20.), 1. / 60.).unwrap();
        let mut onset = Onset::default();
        let hard = onset.update(contact(200.), 1. / 60.).unwrap();
        assert!(hard > soft);
        onset.update(contact(0.), 1. / 60.);
        assert_eq!(onset.update(contact(200.), 1. / 60.), None);
    }
}
