//! Base-disc worker counters 82D8475C..82D84790 and publication
//! 82D82F30..82D82F4C. These count active worker updates, not host frames.
use super::gate::GateState;

/// Fields from one completed physical publication, after template copying.
pub struct PublicationInputs {
    pub filtered: i32,
    pub state_44: u8,
    pub state_45: u8,
    pub motion_1c4: u8,
    pub skeleton_257: u8,
}

/// The duration dependency of worker 82D83620; other scoring is separate.
#[derive(Default, Clone, Copy, Debug)]
pub struct Publication {
    pub gate: GateState,
    pub duration: WipeoutDuration,
}

impl Publication {
    /// Returns this packet's Scoring +3518. 82DBA980..82DBA98C first copies
    /// the scoring template; inactive packets retain its zero duration.
    pub fn advance(&mut self, input: PublicationInputs) -> f32 {
        let actions = self
            .gate
            .advance(input.filtered, input.state_44, input.state_45);
        if actions.reset {
            self.duration = WipeoutDuration::default();
        }
        // 82D837D4 publishes the exit before the inactive worker runs.
        let mut published = if actions.exit_active {
            self.duration.published_duration()
        } else {
            0.0
        };
        if self.gate.active_8b4 != 0 {
            self.duration.advance(input.motion_1c4, input.skeleton_257);
            published = self.duration.published_duration();
        } else if self.gate.secondary_8b5 != 0 && input.motion_1c4 == 0 {
            // 82D83FA0..82D83FA8 gates the inactive worker; its tail at
            // 82D842E8..82D842F8 increments only +7E0.
            self.duration.total_updates = self.duration.total_updates.wrapping_add(1);
        }
        published
    }
}

/// Worker +7E0/+7E4; reset 82D821D8 clears both.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WipeoutDuration {
    pub total_updates: u32,
    pub unsettled_updates: u32,
}

impl WipeoutDuration {
    /// Call only on the active worker path, after its original gate/reset.
    /// Motion +1C4 blocks both increments; Skeleton +257 blocks only +7E4.
    pub fn advance(&mut self, motion_1c4: u8, skeleton_257: u8) {
        if motion_1c4 == 0 {
            self.total_updates = self.total_updates.wrapping_add(1);
            if skeleton_257 == 0 {
                self.unsettled_updates = self.unsettled_updates.wrapping_add(1);
            }
        }
    }

    /// Scoring +3518 (record +4E8): unsigned conversion, binary32 rounding,
    /// then binary32 multiplication by the executable's 1/60 constant.
    /// Publication also runs on worker exit. Resetting this counter does not
    /// clear the separate scoring record; its lifetime belongs to the caller.
    pub fn published_duration(&self) -> f32 {
        self.total_updates as f32 * f32::from_bits(0x3c88_8889)
    }
}

/// Section A presence at 82786A18..82786A38. Preserve the original unordered
/// comparison: positive values include the section; NaN and either zero omit it.
/// Collision +C4 supplies the strength independently of duration.
pub fn channel_5_strength(scoring_3518: f32, collision_c4: f32) -> Option<f32> {
    if scoring_3518 > 0.0 {
        Some(collision_c4)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(filtered: i32, motion: u8, settled: u8) -> PublicationInputs {
        PublicationInputs {
            filtered,
            state_44: 0,
            state_45: 0,
            motion_1c4: motion,
            skeleton_257: settled,
        }
    }

    #[test]
    fn active_entry_exit_and_template_packet_lifetime() {
        let mut worker = Publication::default();
        worker.duration.total_updates = 99;
        assert_eq!(worker.advance(packet(4, 0, 0)).to_bits(), 0x3c88_8889);
        worker.advance(packet(4, 1, 0));
        worker.advance(packet(4, 0, 1));
        assert_eq!(
            worker.duration,
            WipeoutDuration {
                total_updates: 2,
                unsettled_updates: 1
            }
        );
        assert_eq!(
            worker.advance(packet(0, 0, 0)),
            worker.duration.published_duration()
        );
        assert_eq!(worker.advance(packet(0, 0, 0)), 0.0);
        assert_eq!(worker.duration.total_updates, 2);
    }

    #[test]
    fn secondary_counts_without_publication_and_exit_precedes_increment() {
        let mut worker = Publication::default();
        assert_eq!(worker.advance(packet(2, 0, 0)), 0.0);
        assert_eq!(
            worker.duration,
            WipeoutDuration {
                total_updates: 1,
                unsettled_updates: 0
            }
        );
        worker.advance(packet(4, 0, 0));
        assert_eq!(worker.duration.total_updates, 2);
        let before_exit = worker.duration.published_duration();
        assert_eq!(worker.advance(packet(0, 0, 0)), before_exit);
        assert_eq!(worker.duration.total_updates, 3);
        assert_eq!(worker.duration.unsettled_updates, 1);
        worker.advance(packet(2, 1, 0));
        assert_eq!(worker.duration.total_updates, 3);
    }

    #[test]
    fn secondary_reset_precedes_exit_publication() {
        let mut worker = Publication::default();
        worker.advance(packet(4, 0, 0));
        assert_eq!(worker.advance(packet(2, 0, 0)), 0.0);
        assert_eq!(
            worker.duration,
            WipeoutDuration {
                total_updates: 1,
                unsettled_updates: 0
            }
        );
    }

    #[test]
    fn motion_and_settled_flags_gate_independent_wrapping_counters() {
        let mut state = WipeoutDuration {
            total_updates: u32::MAX,
            unsettled_updates: u32::MAX,
        };
        state.advance(255, 0);
        assert_eq!(state.total_updates, u32::MAX);
        assert_eq!(state.unsettled_updates, u32::MAX);
        state.advance(0, 255);
        assert_eq!(state.total_updates, 0);
        assert_eq!(state.unsettled_updates, u32::MAX);
        state.advance(0, 0);
        assert_eq!(state.total_updates, 1);
        assert_eq!(state.unsettled_updates, 0);
    }

    #[test]
    fn publication_matches_native_unsigned_conversion_and_rounding() {
        // Expected words replayed from base-disc 82D82F30..82D82F4C.
        for (total_updates, bits) in [
            (0, 0),
            (1, 0x3c88_8889),
            (60, 0x3f80_0000),
            (0x0100_0001, 0x4888_8889),
            (u32::MAX, 0x4c88_8889),
        ] {
            let state = WipeoutDuration {
                total_updates,
                unsettled_updates: 0,
            };
            assert_eq!(state.published_duration().to_bits(), bits);
        }
    }

    #[test]
    fn section_presence_uses_duration_and_copies_strength_independently() {
        for absent in [0.0, -0.0, -1.0, f32::NEG_INFINITY, f32::NAN] {
            assert_eq!(channel_5_strength(absent, 0.75), None);
        }
        for present in [f32::from_bits(1), 1.0, f32::INFINITY] {
            assert_eq!(
                channel_5_strength(present, -0.0).unwrap().to_bits(),
                0x8000_0000
            );
            assert_eq!(channel_5_strength(present, 0.75), Some(0.75));
        }
        let published = WipeoutDuration {
            total_updates: 1,
            unsettled_updates: 0,
        }
        .published_duration();
        // Worker reset cannot clear an already published, separately owned value.
        assert_eq!(WipeoutDuration::default().published_duration(), 0.0);
        assert_eq!(channel_5_strength(published, 0.75), Some(0.75));
    }
}
