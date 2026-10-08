//! Board-contact publication state: deck8274F578..8274F5EC,
//! grind8274FE90..8274FF58 (base disc).
//! The event selector and audio-manager clock do not own this history.

#[derive(Clone, Debug, Default)]
pub struct DeckStrengthHistory {
    history: [f32; 4],
    next: usize,
    /// Audio+0x24, copied to packet+0x1E4 and snapshot+0x29C.
    pub published: f32,
}

impl DeckStrengthHistory {
    /// Publisher virtual+8, 8274F2B0, reached by the ordinary teleport reset
    /// through82D8E0BC ->82DC7B98 ->82DC7BF8.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Advance once per completed physics publication, including zero-strength
    /// publications. Native initialization8274F330/F398 zeros index and slots.
    pub fn publish(&mut self, strength: f32) {
        self.history[self.next] = strength;
        self.next = (self.next + 1) % self.history.len();
        // Original comparisons scan physical slots, not age order. Seed with
        // slot zero to retain the native behavior for ties and unordered values.
        let mut maximum = self.history[0];
        for &candidate in &self.history[1..] {
            if candidate > maximum {
                maximum = candidate;
            }
        }
        self.published = maximum;
    }
}

/// Audio+1C/+20/+28/+D0. These are published physical inputs, independent
/// of the Contacts rising-edge latch and elapsed-time cooldown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrindPublication {
    pub family: u32,
    pub strength: f32,
    pub surface_tag: u32,
    pub active: bool,
}

impl Default for GrindPublication {
    fn default() -> Self {
        // Publisher reset8274F324/F334/F33C.
        Self {
            family: u32::MAX,
            strength: 0.0,
            surface_tag: 0,
            active: false,
        }
    }
}

impl GrindPublication {
    /// Grinds+13C/+88/+D8/+80/+143 and State+0C/+10, respectively.
    /// Strength is passed through; no host speed conversion belongs here.
    pub fn publish(
        &mut self,
        grinding: bool,
        family: u32,
        surface_tag: u32,
        strength: f32,
        flag_323: bool,
        state_12: u32,
        state_16: u32,
    ) {
        if grinding {
            self.family = family;
            self.surface_tag = surface_tag;
        }
        if strength > 0.0 {
            self.strength = strength;
        }
        self.active = state_12 == 400 || (state_16 == 701 && flag_323);
    }

    /// The same publisher virtual+8 as deck history; ordinary teleport calls it.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grind_publication_retains_contact_inputs_independently_of_active_state() {
        let mut publication = GrindPublication::default();
        publication.publish(true, 2, 15, 0.4, false, 400, 0);
        assert!(publication.active);
        publication.publish(false, 5, 30, 0.0, false, 200, 701);
        assert_eq!(
            (
                publication.family,
                publication.surface_tag,
                publication.strength
            ),
            (2, 15, 0.4)
        );
        assert!(!publication.active);
        publication.publish(false, 5, 30, f32::NAN, true, 200, 701);
        assert!(publication.active);
        assert_eq!(publication.strength, 0.4);
        publication.publish(true, 5, 30, -1.0, false, 400, 701);
        assert_eq!(
            (
                publication.family,
                publication.surface_tag,
                publication.strength
            ),
            (5, 30, 0.4)
        );
        publication.reset();
        assert_eq!(publication, GrindPublication::default());
    }

    #[test]
    fn teleport_reset_clears_peak_slots_and_restarts_the_write_index() {
        let mut history = DeckStrengthHistory::default();
        history.publish(1.0);
        history.publish(0.75);
        history.reset();
        assert_eq!(history.published, 0.0);
        assert_eq!(history.history, [0.0; 4]);
        assert_eq!(history.next, 0);
        history.publish(0.25);
        for _ in 0..3 {
            history.publish(0.0);
        }
        assert_eq!(history.published, 0.25);
        history.publish(0.0);
        assert_eq!(history.published, 0.0);
    }

    #[test]
    fn peak_expires_on_the_fourth_following_publication() {
        let mut history = DeckStrengthHistory::default();
        assert_eq!(history.published, 0.0);
        history.publish(0.75);
        for strength in [0.0, 0.25, 0.0] {
            history.publish(strength);
            assert_eq!(history.published, 0.75);
        }
        history.publish(0.0);
        assert_eq!(history.published, 0.25);
        history.publish(0.0);
        assert_eq!(history.published, 0.25);
        history.publish(0.0);
        assert_eq!(history.published, 0.0);
    }

    #[test]
    fn scan_preserves_slot_zero_on_ties_and_unordered_comparisons() {
        let mut history = DeckStrengthHistory::default();
        history.publish(-0.0);
        assert_eq!(history.published.to_bits(), (-0.0f32).to_bits());
        history.publish(f32::NAN);
        history.publish(0.5);
        assert_eq!(history.published, 0.5);
        history.publish(0.25);
        history.publish(f32::NAN);
        assert!(history.published.is_nan());
    }
}
