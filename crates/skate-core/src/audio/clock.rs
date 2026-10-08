//! Audio-manager phase scheduling, base-disc 82473074..82473120.
//!
//! The caller supplies the original manager delta, enabled state, and force
//! flag. These inputs are not inferred from rendering, physics, or pause state.
//! This does not implement the publication provider or MixMap evaluation.

/// Selected phases in execution order: inputs, then MixMap/output.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UpdatePhases {
    pub inputs: Option<f32>,
    pub mixmap: Option<f32>,
}

/// Fields +1DC/+1E0/+1E4, initialized to zero at 824725B0..824725BC.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioClock {
    pending_inputs: f32,
    pending_mixmap: f32,
    mixmap_next: bool,
}

impl AudioClock {
    pub fn advance(&mut self, delta: f32, enabled: bool, force: bool) -> UpdatePhases {
        if !enabled {
            return UpdatePhases::default();
        }
        let inputs = self.pending_inputs + delta;
        let mixmap = self.pending_mixmap + delta;
        // Original lfd/fcmpu compares the promoted f32 delta with a binary64
        // constant; retain those operand widths.
        if f64::from(delta) > 0.02_f64 || force {
            *self = Self::default();
            UpdatePhases {
                inputs: Some(inputs),
                mixmap: Some(mixmap),
            }
        } else if self.mixmap_next {
            self.pending_inputs = delta;
            self.pending_mixmap = 0.0;
            self.mixmap_next = false;
            UpdatePhases {
                inputs: None,
                mixmap: Some(mixmap),
            }
        } else {
            self.pending_inputs = 0.0;
            self.pending_mixmap = delta;
            self.mixmap_next = true;
            UpdatePhases {
                inputs: Some(inputs),
                mixmap: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixty_hz_alternates_phases_and_keeps_the_first_input_interval() {
        let mut clock = AudioClock::default();
        let delta = 1.0_f32 / 60.0;
        for tick in 0..60 {
            let phases = clock.advance(delta, true, false);
            if tick % 2 == 0 {
                assert_eq!(
                    phases.inputs,
                    Some(if tick == 0 { delta } else { delta + delta })
                );
                assert_eq!(phases.mixmap, None);
            } else {
                assert_eq!(phases.inputs, None);
                assert_eq!(phases.mixmap, Some(delta + delta));
            }
        }
    }

    #[test]
    fn alternation_keeps_independent_elapsed_values() {
        let mut clock = AudioClock::default();
        assert_eq!(
            clock.advance(0.004, true, false),
            UpdatePhases {
                inputs: Some(0.004),
                mixmap: None,
            }
        );
        assert_eq!(
            clock.advance(0.006, true, false),
            UpdatePhases {
                inputs: None,
                mixmap: Some(0.004_f32 + 0.006),
            }
        );
        assert_eq!(
            clock.advance(0.005, true, false),
            UpdatePhases {
                inputs: Some(0.006_f32 + 0.005),
                mixmap: None,
            }
        );
        assert_eq!(
            clock.advance(0.008, true, true),
            UpdatePhases {
                inputs: Some(0.008),
                mixmap: Some(0.005_f32 + 0.008),
            }
        );
        assert_eq!(
            clock.advance(0.003, true, false),
            UpdatePhases {
                inputs: Some(0.003),
                mixmap: None,
            }
        );
    }

    #[test]
    fn disabled_updates_preserve_pending_time_and_phase_even_when_forced() {
        let mut clock = AudioClock::default();
        clock.advance(0.01, true, false);
        let saved = clock;
        assert_eq!(clock.advance(4.0, false, true), UpdatePhases::default());
        assert_eq!(clock, saved);
        assert_eq!(
            clock.advance(0.005, true, false),
            UpdatePhases {
                inputs: None,
                mixmap: Some(0.01_f32 + 0.005),
            }
        );
    }

    #[test]
    fn binary64_threshold_and_long_updates_reset_alternation() {
        let mut clock = AudioClock::default();
        let below = f32::from_bits(0x3ca3_d70a);
        let above = f32::from_bits(0x3ca3_d70b);
        assert!(f64::from(below) < 0.02 && f64::from(above) > 0.02);
        assert_eq!(
            clock.advance(below, true, false),
            UpdatePhases {
                inputs: Some(below),
                mixmap: None,
            }
        );
        assert_eq!(
            clock.advance(above, true, false),
            UpdatePhases {
                inputs: Some(above),
                mixmap: Some(below + above),
            }
        );
        assert_eq!(clock, AudioClock::default());
    }

    #[test]
    fn zero_delta_still_advances_the_enabled_phase() {
        let mut clock = AudioClock::default();
        assert_eq!(
            clock.advance(0.0, true, false),
            UpdatePhases {
                inputs: Some(0.0),
                mixmap: None,
            }
        );
        assert_eq!(
            clock.advance(0.0, true, false),
            UpdatePhases {
                inputs: None,
                mixmap: Some(0.0),
            }
        );
    }
}
