//! Ordinary deck collision records, base-disc824AAE98..824AB1EC.
//! Publication history and audio-manager scheduling belong to separate owners.
use super::body_impact::{ABSENT, Band, BodyError, Materials, Record, truncate};

#[derive(Clone, Copy, Debug)]
pub struct DeckFrame {
    pub strength: f32,
    /// Collision+0x0C, converted by8277EF94..EFBC to snapshot+0x294.
    pub surface_tag: i32,
    pub off_board: bool,
    pub state_2a4: bool,
    pub publication_ratio: f32,
    /// Contacts owner byte+0x48 and whether word+0x40 is zero.
    pub owner: (u8, bool),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::body_impact::{Intensity, Profile, Volume};

    fn materials() -> Materials {
        let mut profiles = vec![None; ABSENT as usize];
        for (id, level) in [(2, 800), (0x5f, 400), (0x71, 200)] {
            profiles[id] = Some(Profile {
                intensity: Intensity {
                    minimum: 0.125,
                    medium: 0.25,
                    hard: 0.5,
                    maximum: 1.0,
                },
                volume: Volume {
                    soft: [[level, level]; 3],
                    medium: [[level, level]; 3],
                    hard: [level, level],
                    hard_layer_scale: 0.5,
                },
            });
        }
        Materials::new(profiles, [0; 95]).unwrap()
    }
    fn frame() -> DeckFrame {
        DeckFrame {
            strength: 0.75,
            surface_tag: 3,
            off_board: false,
            state_2a4: false,
            publication_ratio: 0.5,
            owner: (7, true),
        }
    }

    #[test]
    fn hard_pair_layers_keep_order_flags_and_native_cooldown() {
        let mut deck = DeckImpacts::default();
        let m = materials();
        let mut frame = frame();
        let records = deck.update(&frame, &m, 6).unwrap();
        assert_eq!(
            records,
            [
                Record {
                    materials: [0x5f, 2],
                    categories: [2, 2],
                    levels: [400, 800],
                    flags: [0, 7, 1]
                },
                Record {
                    materials: [0x5f, 2],
                    categories: [1, 1],
                    levels: [200, 400],
                    flags: [0, 0, 1]
                },
            ]
        );
        assert_eq!(deck.cooldown(), 5.5);
        frame.strength = 0.0;
        frame.publication_ratio = 4.0;
        for _ in 0..6 {
            assert!(deck.update(&frame, &m, 6).unwrap().is_empty());
        }
        assert_eq!(deck.cooldown(), -0.5); // No zero clamp.
        frame.strength = 0.125; // Minimum inclusive; no hard overlay.
        frame.off_board = true;
        let records = deck.update(&frame, &m, 6).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].materials, [0x71, 2]);
        assert_eq!(records[0].categories, [0, 0]);
        assert_eq!(records[0].levels, [200, 800]);
        assert_eq!(deck.cooldown(), 5.0);
    }

    #[test]
    fn state_flag_selects_tumble_and_missing_materials_leave_state_unchanged() {
        let m = materials();
        let mut deck = DeckImpacts::default();
        let mut frame = frame();
        frame.strength = 0.1;
        assert!(deck.update(&frame, &m, 6).unwrap().is_empty());
        assert_eq!(deck.cooldown(), 0.0);
        frame.strength = 0.75;
        frame.surface_tag = 4;
        assert_eq!(
            deck.update(&frame, &m, 6),
            Err(BodyError::MissingProfile(3))
        );
        assert_eq!(deck.cooldown(), 0.0);
        frame.surface_tag = 0;
        frame.state_2a4 = true;
        frame.owner = (7, false);
        let records = deck.update(&frame, &m, 0).unwrap();
        assert_eq!(records[0].materials, [0x71, ABSENT]);
        assert_eq!(records[0].categories, [2, 3]);
        assert_eq!(records[0].flags, [0, 7, 0]);
        frame.publication_ratio = f32::NAN;
        assert_eq!(deck.update(&frame, &m, 6), Err(BodyError::InvalidInput));
        assert_eq!(deck.cooldown(), 0.0);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeckImpacts {
    /// Contacts+0x124; only construction resets this timer.
    cooldown: f32,
}

impl DeckImpacts {
    pub fn cooldown(&self) -> f32 {
        self.cooldown
    }

    /// `cooldown` is aud_collisions/default Hash_27D3C5DC3282B59D (Int32).
    /// Like the body port, errors leave state unchanged.
    pub fn update(
        &mut self,
        frame: &DeckFrame,
        materials: &Materials,
        cooldown: i32,
    ) -> Result<Vec<Record>, BodyError> {
        if !frame.strength.is_finite() || !frame.publication_ratio.is_finite() {
            return Err(BodyError::InvalidInput);
        }
        let mut timer = self.cooldown;
        let mut records = Vec::new();
        if frame.strength > 0.0 && !(timer > 0.0) {
            let board = if frame.off_board || frame.state_2a4 {
                0x71
            } else {
                0x5f
            };
            let surface = if (1..=ABSENT + 1).contains(&frame.surface_tag) {
                frame.surface_tag - 1
            } else {
                ABSENT
            };
            let board_band = materials.classify(board, frame.strength)?;
            let surface_band = if surface < ABSENT {
                materials.classify(surface, frame.strength)?
            } else {
                Band::SILENT
            };
            if board_band.category != 3 || surface_band.category != 3 {
                timer = cooldown as f32;
                let levels = [
                    materials.level(board, surface, board_band, frame.strength)?,
                    materials.level(surface, board, surface_band, frame.strength)?,
                ];
                let tail = u8::from(frame.owner.0 != 0 && frame.owner.1);
                records.push(Record {
                    materials: [board, surface],
                    categories: [board_band.category, surface_band.category],
                    levels,
                    flags: [0, frame.owner.0, tail],
                });
                let overlay = [board_band.category, surface_band.category]
                    .map(|category| if category == 2 { 1 } else { 3 });
                if overlay != [3, 3] {
                    records.push(Record {
                        materials: [board, surface],
                        categories: overlay,
                        levels: [
                            truncate(materials.hard_layer_scale(board)? * levels[0] as f32)?,
                            truncate(materials.hard_layer_scale(surface)? * levels[1] as f32)?,
                        ],
                        flags: [0, 0, tail],
                    });
                }
            }
        }
        if timer > 0.0 {
            // Assignment and decrement occur in the same call, without a
            // zero clamp. Negative ratios increase the native timer.
            let step = if frame.publication_ratio > 1.0 {
                1.0
            } else {
                frame.publication_ratio
            };
            timer -= step;
        }
        self.cooldown = timer;
        Ok(records)
    }
}
