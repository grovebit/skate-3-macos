//! Contacts grind onset824A70C0..70F0/824A8F78..91C4 (base disc).
//! Cooldown824AC8E8..C900 runs separately in the output callback.
use super::board_contact::GrindPublication;
use super::body_impact::{ABSENT, Band, BodyError, Materials, Record};

#[derive(Clone, Copy, Debug, Default)]
pub struct GrindStarts {
    previous: bool,
    cooldown: f32,
}

impl GrindStarts {
    pub fn cooldown(&self) -> f32 {
        self.cooldown
    }

    /// Input callback. Bands are aud_rails/default086B66C3D4FFEE8F and
    /// B2ACAFDBCD963C93. Owner is byte48 and whether word40 equals zero.
    pub fn input(
        &mut self,
        frame: GrindPublication,
        bands: [f32; 2],
        owner: (u8, bool),
        materials: &Materials,
    ) -> Result<Option<Record>, BodyError> {
        if !frame.strength.is_finite() || bands.iter().any(|x| !x.is_finite()) {
            return Err(BodyError::InvalidInput);
        }
        let mut record = None;
        if frame.active && !self.previous && !(self.cooldown > 0.0) {
            let board = if matches!(frame.family, 1 | 2 | 5) {
                0x5f
            } else {
                0x60
            };
            let surface = if (1..=ABSENT as u32 + 1).contains(&frame.surface_tag) {
                frame.surface_tag as i32 - 1
            } else {
                ABSENT
            };
            let surface = if surface == ABSENT { 0x0a } else { surface };
            let band = if frame.strength > bands[0] {
                Band {
                    category: 1,
                    lower: bands[0],
                    upper: bands[1],
                }
            } else {
                Band {
                    category: 0,
                    lower: 0.0,
                    upper: bands[0],
                }
            };
            record = Some(Record {
                materials: [board, surface],
                categories: [band.category; 2],
                levels: [
                    materials.level(board, surface, band, frame.strength)?,
                    materials.level(surface, board, band, frame.strength)?,
                ],
                flags: [0, 0, u8::from(owner.0 != 0 && owner.1)],
            });
            self.cooldown = 0.5; // 82099700. Always set after enqueue, even at zero level.
        }
        self.previous = frame.active; // Blocked rising edges are consumed.
        Ok(record)
    }

    /// Output callback uses group+3C: the last input-phase elapsed seconds,
    /// retained across output-only calls, not the output-phase elapsed value.
    pub fn output(&mut self, input_elapsed: f32) -> Result<(), BodyError> {
        if !input_elapsed.is_finite() {
            return Err(BodyError::InvalidInput);
        }
        if self.cooldown > 0.0 {
            self.cooldown -= input_elapsed;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::body_impact::{Intensity, Profile, Volume};
    fn materials() -> Materials {
        let profile = Profile {
            intensity: Intensity {
                minimum: 0.0,
                medium: 0.1,
                hard: 0.2,
                maximum: 1.0,
            },
            volume: Volume {
                soft: [[10, 20]; 3],
                medium: [[30, 40]; 3],
                hard: [50, 60],
                hard_layer_scale: 1.0,
            },
        };
        Materials::new(vec![Some(profile); ABSENT as usize], [0; 95]).unwrap()
    }
    #[test]
    fn blocked_edge_stays_consumed_after_expiry_until_another_rising_edge() {
        let m = materials();
        let mut g = GrindStarts::default();
        let mut f = GrindPublication {
            active: true,
            family: 2,
            strength: 0.25,
            surface_tag: 0,
        };
        let r = g.input(f, [0.25, 0.5], (7, true), &m).unwrap().unwrap();
        assert_eq!(r.materials, [0x5f, 0x0a]);
        assert_eq!(r.categories, [0, 0]); // Upper soft boundary is inclusive.
        assert_eq!(r.flags, [0, 0, 1]);
        f.active = false;
        g.input(f, [0.25, 0.5], (0, false), &m).unwrap();
        f.active = true;
        assert!(g.input(f, [0.25, 0.5], (0, false), &m).unwrap().is_none());
        g.output(0.75).unwrap();
        assert_eq!(g.cooldown(), -0.25);
        assert!(g.input(f, [0.25, 0.5], (0, false), &m).unwrap().is_none());
        f.active = false;
        g.input(f, [0.25, 0.5], (0, false), &m).unwrap();
        f.active = true;
        f.strength = f32::from_bits(0x3e80_0001);
        assert_eq!(
            g.input(f, [0.25, 0.5], (0, false), &m)
                .unwrap()
                .unwrap()
                .categories,
            [1, 1]
        );
    }

    #[test]
    fn output_uses_last_input_elapsed_when_manager_phases_alternate() {
        use crate::audio::clock::AudioClock;
        let mut clock = AudioClock::default();
        let mut g = GrindStarts::default();
        let frame = GrindPublication {
            active: true,
            family: 0,
            strength: 0.5,
            surface_tag: 3,
        };
        let first = clock.advance(0.01, true, false);
        let last_input = first.inputs.unwrap();
        g.input(frame, [0.25, 0.5], (0, false), &materials())
            .unwrap();
        assert_eq!(g.cooldown(), 0.5);
        let second = clock.advance(0.005, true, false);
        assert!(second.inputs.is_none());
        assert_eq!(second.mixmap, Some(0.01_f32 + 0.005));
        g.output(last_input).unwrap();
        assert_eq!(g.cooldown(), 0.5_f32 - 0.01);
        assert_ne!(g.cooldown(), 0.5_f32 - second.mixmap.unwrap());
    }
}
