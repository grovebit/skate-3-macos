//! Wheel-contact selection from base-disc 824A6578/824A6BE0/824A8288.
//! The adapter supplies completed physical contacts and the physical air timer.
//! Special-mode overrides, remote-listener overrides and catch events are omitted.

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Event {
    pub index: usize,
    pub mode: usize,
    pub gain_mode: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Input {
    pub touching: [bool; 4],
    pub air_time: f32,
    pub airborne: bool,
    pub mode: usize,
    pub thresholds: [f32; 2],
}

#[derive(Default)]
pub(super) struct Tracker {
    landing_armed: bool,
    latched: [bool; 4],
    previous_scalar: [f32; 4],
    categories: [usize; 4],
    overlay: bool,
}

impl Tracker {
    pub fn prime(&mut self, input: Input) {
        self.latched = input.touching;
        self.previous_scalar = [scalar(input); 4];
        self.landing_armed = input.airborne && !input.touching.iter().any(|v| *v);
    }

    pub fn update(&mut self, input: Input) -> [Option<Event>; 2] {
        // Host adapter guard, not a recovered native rule: raw solver contact
        // loss while rolling is not the original conditioned contact input.
        // Arm a landing only during physical flight, then retain partial-contact
        // grouping until the board settles. No minimum height/impulse is used.
        self.landing_armed |= input.airborne && !input.touching.iter().any(|v| *v);
        if !self.landing_armed {
            self.prime(input);
            return [None; 2];
        }
        let value = scalar(input);
        let mut new = [false; 4];
        let mut category = 0;
        for i in 0..4 {
            // 824A0248 classifies last frame's scalar before retaining this one.
            if value > 0. || input.touching[i] {
                self.categories[i] = classify(self.previous_scalar[i], input.thresholds);
                self.previous_scalar[i] = value;
            }
            if input.touching[i] {
                new[i] = !self.latched[i];
                if new[i] {
                    category = category.max(self.categories[i]);
                }
            } else {
                self.latched[i] = false;
            }
        }
        let count = new.iter().filter(|&&v| v).count();
        let old = self.latched.iter().filter(|&&v| v).count();
        if count == 0 {
            return [None; 2];
        }
        // Native latch groups avoid a second impact as the last wheel settles.
        let (group, all, guard_overlay) = match (count, old) {
            (4, _) => (0, true, false),
            (3, _) => (0, true, true),
            (1, 3) => {
                self.latched = [true; 4];
                self.landing_armed = false;
                return [None; 2];
            }
            (1 | 2, 2) => (2, count == 1, true),
            (1, _) => (3, false, old == 1),
            (2, 0) => (1, false, false),
            _ => (0, true, true),
        };
        let extra = category > 1 && (!guard_overlay || !self.overlay);
        self.overlay = matches!(group, 1 | 3) && category > 1;
        for i in 0..4 {
            self.latched[i] |= all || new[i];
        }
        if self.latched.iter().all(|v| *v) {
            self.landing_armed = false;
        }
        let event = |category| Event {
            index: group * 3 + category,
            mode: if group <= 2 && category >= 2 {
                0
            } else {
                input.mode
            },
            gain_mode: input.mode,
        };
        [Some(event(category.min(1))), extra.then(|| event(2))]
    }
}

fn scalar(input: Input) -> f32 {
    if input.airborne && input.air_time.is_finite() {
        (input.air_time * 0.5).clamp(0., 1.)
    } else {
        0.
    }
}

fn classify(value: f32, thresholds: [f32; 2]) -> usize {
    if value >= thresholds[1] {
        2
    } else if value >= thresholds[0] {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(touching: [bool; 4], air_time: f32) -> Input {
        Input {
            touching,
            air_time,
            airborne: air_time > 0.,
            mode: 3,
            thresholds: [0.31, 0.5],
        }
    }

    #[test]
    fn low_landings_sound_once_without_an_impulse_gate() {
        let mut tracker = Tracker::default();
        tracker.prime(input([false; 4], 0.1));
        let events = tracker.update(input([true; 4], 0.));
        assert_eq!(events[0].unwrap().index, 0);
        assert_eq!(events[0].unwrap().mode, 3);
        assert!(events[1].is_none());
        for _ in 0..120 {
            assert!(
                tracker
                    .update(input([true; 4], 0.))
                    .iter()
                    .all(Option::is_none)
            );
        }
    }

    #[test]
    fn hard_landings_layer_a_heavy_event_and_keep_original_gain_mode() {
        let mut tracker = Tracker::default();
        tracker.prime(input([false; 4], 1.));
        let events = tracker.update(input([true; 4], 0.));
        assert_eq!(events[0].unwrap().index, 1);
        let heavy = events[1].unwrap();
        assert_eq!((heavy.index, heavy.mode, heavy.gain_mode), (2, 0, 3));
        assert_eq!(classify(0.31, [0.31, 0.5]), 1);
        assert_eq!(classify(0.5, [0.31, 0.5]), 2);
    }

    #[test]
    fn partial_landings_use_contact_groups_and_suppress_last_wheel() {
        let mut tracker = Tracker::default();
        tracker.prime(input([false; 4], 0.1));
        let first = tracker.update(input([true, true, false, false], 0.));
        assert_eq!(first[0].unwrap().index, 3);
        let next = tracker.update(input([true, true, true, false], 0.));
        assert_eq!(next[0].unwrap().index, 6);
        assert!(
            tracker
                .update(input([true; 4], 0.))
                .iter()
                .all(Option::is_none)
        );
        tracker.update(input([false; 4], 0.1));
        assert_eq!(
            tracker.update(input([true, false, false, false], 0.))[0]
                .unwrap()
                .index,
            9
        );
    }

    #[test]
    fn ground_contact_chatter_stays_quiet_but_another_low_drop_sounds() {
        let mut tracker = Tracker::default();
        tracker.prime(input([true; 4], 0.));
        for _ in 0..60 {
            for touching in [[false; 4], [true, false, false, false], [true; 4]] {
                assert!(
                    tracker
                        .update(input(touching, 0.))
                        .iter()
                        .all(Option::is_none)
                );
            }
        }
        for _ in 0..2 {
            tracker.update(input([false; 4], 0.01));
            assert_eq!(tracker.update(input([true; 4], 0.))[0].unwrap().index, 0);
        }
    }
}
