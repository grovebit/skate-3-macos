//! Wipeout worker gate selection, base-disc 82D8362C..82D83760.
//! Call once at the original publication boundary; elapsed is not host time.

#[derive(Default, Clone, Copy, Debug)]
pub struct GateState {
    pub active_8b4: u8,
    pub secondary_8b5: u8,
    pub elapsed_8bc: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Actions {
    pub reset: bool,
    /// The primary reset additionally captures the vector and score inputs.
    pub capture_primary: bool,
    /// 82D837B8 contributes authored graphs on entry, before active refresh.
    pub enter_active: bool,
    /// 82D837D4 publishes exit scoring when the new primary gate is zero.
    pub exit_active: bool,
}

impl GateState {
    pub fn advance(&mut self, filtered: i32, state_44: u8, state_45: u8) -> Actions {
        let old_active = self.active_8b4;
        let old_secondary = self.secondary_8b5;
        self.active_8b4 = u8::from(filtered == 4 && state_44 == 0 && state_45 == 0);
        let airborne = filtered == 2 || filtered == 7;
        self.elapsed_8bc = if airborne {
            0.0
        } else {
            self.elapsed_8bc + f32::from_bits(0x3c88_8889)
        };
        self.secondary_8b5 = u8::from(
            airborne || (old_secondary != 0 && self.elapsed_8bc < f32::from_bits(0x3dcc_cccd)),
        );
        let capture_primary = old_secondary == 0 && old_active == 0 && self.active_8b4 != 0;
        Actions {
            reset: (old_secondary == 0 && self.secondary_8b5 != 0) || capture_primary,
            capture_primary,
            enter_active: old_active != self.active_8b4 && self.active_8b4 != 0,
            exit_active: old_active != self.active_8b4 && self.active_8b4 == 0,
        }
    }
}
