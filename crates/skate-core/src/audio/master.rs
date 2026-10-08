//! Master publication-speed inputs, base-disc 824C2E58..824C3010.
//! The publication header comes from the original provider, not host delta.

/// Audio fields in SKATER_PROFILE, constructed at 824E8888 and stored through
/// 824E9994..824E9998 in the global service's +1C slot. Profile loading and UI
/// ownership remain separate; these are not the host playback gain setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfileVolumes {
    /// Profile +80, menu table index 7 (ID_GAMESETTINGS_SFXVOLUME).
    pub effects: f32,
    /// Profile +84, menu table index 8 (ID_GAMESETTINGS_DIALOGVOLUME).
    pub dialogue: f32,
    /// Profile +88, menu table index 9 (ID_GAMESETTINGS_MUSICVOLUME).
    pub music: f32,
}

impl Default for ProfileVolumes {
    /// 824E8950..824E8964 calls 824ED4D8 on profile +80; the three floats
    /// are initialized to original constant 82314D90 (1.0).
    fn default() -> Self {
        Self {
            effects: 1.0,
            dialogue: 1.0,
            music: 1.0,
        }
    }
}

impl ProfileVolumes {
    /// Field-changing slice of the original settings menu 825F3F48..825F418C.
    /// Original event 14 lowers and 15 raises. The UI notifications, profile
    /// persistence and other menu events are outside this helper.
    pub fn apply_menu_event(&mut self, setting_index: u32, event: u32) -> Option<f32> {
        let direction: f32 = match event {
            14 => -1.0,
            15 => 1.0,
            _ => return None,
        };
        let field = match setting_index {
            7 => &mut self.effects,
            8 => &mut self.dialogue,
            9 => &mut self.music,
            _ => return None,
        };
        if !field.is_finite() {
            return None;
        }
        // FMADDS f0,f31,step,current. Keep the fused operation and strict
        // threshold; a separately rounded add/multiply changes some inputs.
        let mut value = direction
            .mul_add(f32::from_bits(0x3dcccccd), *field)
            .clamp(0.0, 1.0);
        if value < f32::from_bits(0x3d4ccccd) {
            value = 0.0;
        }
        *field = value;
        Some(value)
    }

    pub fn publication(self) -> Option<SettingsInputs> {
        SettingsInputs::evaluate([self.music, self.effects, self.dialogue])
    }
}

/// Master824C2D48..824C2E58: settings-object88/80/84 -> slots1/2/3.
/// Raw converted values remain on the Master object before controller clamping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsInputs {
    pub raw: [i32; 3],
    pub levels: [u16; 3],
}

impl SettingsInputs {
    /// Values must come from the original settings producer. They are not
    /// inferred from a host master slider or a physical ragdoll state.
    /// Invalid/out-of-range PPC integer conversions are outside this port.
    pub fn evaluate(settings_88_80_84: [f32; 3]) -> Option<Self> {
        let mut raw = [0; 3];
        for (out, setting) in raw.iter_mut().zip(settings_88_80_84) {
            let scaled = setting * 32767.0; //8217291C
            *out = super::voice::truncate(scaled).ok()?;
        }
        Some(Self {
            raw,
            levels: raw.map(|value| value.clamp(0, 32767) as u16),
        })
    }

    /// A missing bound controller suppresses writes, but not evaluation or
    /// storage of the raw values on the original Master object.
    pub fn write(self, master: Option<&mut [u32; 16]>) {
        if let Some(master) = master {
            for (slot, level) in master[1..4].iter_mut().zip(self.levels) {
                *slot = u32::from(level);
            }
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    #[test]
    fn constructed_timer_publishes_once_then_follows_replay_transitions() {
        let mut timer = TransitionTimer::default();
        let mut words = [0; 16];
        timer.publish(0, 1. / 30., Some(false), 1., &mut words);
        assert_eq!((words[6], words[12]), (32767, 0));
        assert!(timer.remaining_40 < 0.);
        timer.publish(0, 1. / 30., Some(false), 1., &mut words);
        assert_eq!(words[6], 0);
        timer.publish(0, 1. / 30., Some(true), 0., &mut words);
        assert_eq!(words[6], 32767);
        timer.publish(2, 0.5, Some(false), 1., &mut words);
        assert_eq!((words[6], words[12]), (32767, 32767));
        assert_eq!(timer.remaining_40, f32::from_bits(0x3f4ccccd) - 0.5);
        // Still not below zero: publishes again, then goes negative.
        timer.publish(2, 0.5, Some(false), 1., &mut words);
        assert_eq!(words[6], 32767);
        timer.publish(2, 0.5, Some(false), 1., &mut words);
        assert_eq!(words[6], 0);
        timer.publish(0, 0.25, None, 1., &mut words);
        assert_eq!((words[6], timer.remaining_40), (32767, 0.25));
        assert_eq!(state_slot0(Some(true)), 32767);
        assert_eq!(state_slot0(None), 0);
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    #[test]
    fn settings_clamp_controller_words_but_retain_raw_object_values() {
        let input = SettingsInputs::evaluate([-0.5, 0.5, 2.0]).unwrap();
        assert_eq!(input.raw, [-16383, 16383, 65534]);
        assert_eq!(input.levels, [0, 16383, 32767]);
        let mut words = [42; 16];
        input.write(Some(&mut words));
        assert_eq!(&words[1..4], &[0, 16383, 32767]);
        assert_eq!(words[0], 42);
        assert_eq!(&words[4..], &[42; 12]);
        input.write(None);
        assert_eq!(SettingsInputs::evaluate([f32::NAN, 1.0, 1.0]), None);
        assert_eq!(SettingsInputs::evaluate([f32::INFINITY, 1.0, 1.0]), None);
    }

    #[test]
    fn profile_defaults_menu_boundaries_and_publication_order_follow_original() {
        let mut profile = ProfileVolumes::default();
        assert_eq!(profile.publication().unwrap().levels, [32767; 3]);
        profile.effects = 0.5;
        profile.dialogue = 0.25;
        profile.music = 0.75;
        assert_eq!(profile.publication().unwrap().levels, [24575, 16383, 8191]);
        assert_eq!(profile.apply_menu_event(7, 15), Some(0.6));
        profile.dialogue = 0.125;
        assert_eq!(profile.apply_menu_event(8, 14), Some(0.0));
        profile.music = 1.0;
        assert_eq!(profile.apply_menu_event(9, 15), Some(1.0));
        let before = profile;
        assert_eq!(profile.apply_menu_event(10, 15), None);
        assert_eq!(profile.apply_menu_event(7, 3), None);
        assert_eq!(profile, before);
    }
}
/// Master slot 0 (824C2CF0..824C2D44): 8277ADC0 queries the primary
/// provider's virtual +10 when a context exists. Both ordinary providers
/// install 82A248A0, which returns zero.
pub fn state_slot0(context_query: Option<bool>) -> u32 {
    if context_query == Some(true) {
        32767
    } else {
        0
    }
}

/// Master +3C (FE state 2 latch) and +40 (transition timer). Constructor
/// 824C2B28..824C2B60 stores 0 and 0.0, so the first update publishes slot 6.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TransitionTimer {
    pub replay_3c: u8,
    pub remaining_40: f32,
}

impl TransitionTimer {
    /// 824C3010..824C3144: slot 12 is FE state 2 (replay2). Entering it
    /// sets the timer to 0.8 (8208821C) and leaving it to 0.5 (82099700).
    /// A timer not below zero publishes slot 6 and loses this update's
    /// incoming seconds (f27), with no clamp. Otherwise slot 6 follows the
    /// context query at a zero publication speed. Unordered timers publish.
    pub fn publish(
        &mut self,
        fe_state: i32,
        elapsed: f32,
        context_query: Option<bool>,
        speed: f32,
        master: &mut [u32; 16],
    ) {
        let replay = fe_state == 2;
        master[12] = if replay { 32767 } else { 0 };
        if replay && self.replay_3c == 0 {
            self.remaining_40 = f32::from_bits(0x3f4ccccd);
        } else if !replay && self.replay_3c != 0 {
            self.remaining_40 = 0.5;
        }
        self.replay_3c = u8::from(replay);
        // fcmpu/blt: only an ordered "below zero" takes the query branch.
        master[6] = if !(self.remaining_40 < 0.0) {
            self.remaining_40 -= elapsed;
            32767
        } else if context_query == Some(true) && speed == 0.0 {
            32767
        } else {
            0
        };
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationSpeed {
    pub low: u16,
    pub high: u16,
    pub below_normal: u16,
}
impl PublicationSpeed {
    /// `treatment_scale` is the resolved aud_general/treatments setting
    /// Hash_A12258EB71B6937A. It applies only with context, manager byte34B set,
    /// and the unscaled publication header below one. Unsupported nonfinite
    /// inputs fail; no substitute speed or tuning value is inferred.
    pub fn evaluate(
        header: Option<f32>,
        treatment_enabled: bool,
        treatment_scale: f32,
    ) -> Option<Self> {
        let speed = Self::speed(header, treatment_enabled, treatment_scale)?;
        // Constants: 8217291C=32767, 820ED39C=100, 822F40B0=8191.75.
        let low = (speed * 32767.).clamp(100., 32767.) as u16;
        let high = if speed > 1. {
            ((speed - 1.) * 8191.75).clamp(0., 32767.) as u16
        } else {
            0
        };
        Some(Self {
            low,
            high,
            below_normal: if speed < 1. { 32767 } else { 0 },
        })
    }
    /// f31 at 824C2E58..824C2EE4, also read by the slot-6 branch.
    pub fn speed(
        header: Option<f32>,
        treatment_enabled: bool,
        treatment_scale: f32,
    ) -> Option<f32> {
        let mut speed = header.unwrap_or(1.);
        if !speed.is_finite() {
            return None;
        }
        if header.is_some() && treatment_enabled && speed < 1. {
            speed *= treatment_scale;
            if !speed.is_finite() {
                return None;
            }
        }
        Some(speed)
    }
    /// Update only the original Master slots. Settings and other flags have
    /// their own producers; preserving their words is intentional.
    pub fn write(self, master: &mut [u32; 16]) {
        master[4] = u32::from(self.low);
        master[5] = u32::from(self.high);
        master[11] = u32::from(self.below_normal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_context_normal_slow_and_fast_publications_follow_original_bounds() {
        assert_eq!(
            PublicationSpeed::evaluate(None, true, f32::NAN),
            Some(PublicationSpeed {
                low: 32767,
                high: 0,
                below_normal: 0
            })
        );
        assert_eq!(
            PublicationSpeed::evaluate(Some(0.), false, 1.),
            Some(PublicationSpeed {
                low: 100,
                high: 0,
                below_normal: 32767
            })
        );
        assert_eq!(
            PublicationSpeed::evaluate(Some(0.5), false, 1.),
            Some(PublicationSpeed {
                low: 16383,
                high: 0,
                below_normal: 32767
            })
        );
        assert_eq!(
            PublicationSpeed::evaluate(Some(2.), false, 1.),
            Some(PublicationSpeed {
                low: 32767,
                high: 8191,
                below_normal: 0
            })
        );
        assert_eq!(
            PublicationSpeed::evaluate(Some(5.), false, 1.)
                .unwrap()
                .high,
            32767
        );
    }
    #[test]
    fn treatment_branch_uses_original_header_but_outputs_use_scaled_value() {
        assert_eq!(
            PublicationSpeed::evaluate(Some(0.5), true, 4.),
            PublicationSpeed::evaluate(Some(2.), false, 1.)
        );
        assert_eq!(
            PublicationSpeed::evaluate(Some(2.), true, f32::NAN),
            PublicationSpeed::evaluate(Some(2.), false, 1.)
        );
        assert_eq!(PublicationSpeed::evaluate(Some(0.5), true, f32::NAN), None);
        assert_eq!(PublicationSpeed::evaluate(Some(f32::NAN), false, 1.), None);
        let mut words = [42; 16];
        PublicationSpeed::evaluate(Some(2.), false, 1.)
            .unwrap()
            .write(&mut words);
        assert_eq!(words[4], 32767);
        assert_eq!(words[5], 8191);
        assert_eq!(words[11], 0);
        assert_eq!(words[0], 42);
        assert_eq!(words[6], 42);
    }
}
