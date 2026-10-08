//! Collision pitch, base-disc 824C0158, 8249D0A8 and 824C03D4..824C0408.
//! Lookup tables are supplied from the user's owned executable, not generated
//! with powf: table rounding and the negative branch's divisions are observable.
use super::voice::{VoiceError, controller_word, truncate};

pub struct PitchTables {
    semitones: [f32; 12],
    cents: [f32; 100],
}

impl PitchTables {
    /// Tables at 82FBCD9C (48 bytes) and 82FBCDD0 (400 bytes). The word
    /// between them is not an entry and must not be included.
    pub fn from_be_bytes(semitones: &[u8], cents: &[u8]) -> Option<Self> {
        fn read<const N: usize>(bytes: &[u8]) -> Option<[f32; N]> {
            if bytes.len() != N * 4 {
                return None;
            }
            let values = std::array::from_fn(|i| {
                f32::from_be_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
            });
            values
                .iter()
                .all(|v| v.is_finite() && *v > 0.)
                .then_some(values)
        }
        Some(Self {
            semitones: read(semitones)?,
            cents: read(cents)?,
        })
    }

    /// 82923D88, restricted to the signed halfword domain used by this caller.
    pub fn ratio(&self, cents: i16) -> f32 {
        let magnitude = i32::from(cents).unsigned_abs() as usize;
        let remainder = magnitude % 1200;
        let mut octaves = 1.0f32;
        for _ in 0..magnitude / 1200 {
            octaves *= 2.;
        }
        let semitone = self.semitones[remainder / 100];
        let cent = self.cents[remainder % 100];
        if cents >= 0 {
            (cent * semitone) * octaves
        } else {
            ((1. / semitone) / octaves) * (1. / cent)
        }
    }
}

/// The class is the material +48 value resolved by 82484E10, not the
/// counterpart surface class. IDs >=8F bypass that material lookup.
pub fn pitch_slot(material: i32, material_class: i32) -> usize {
    if material < 0x8f && material_class == 9 {
        22
    } else {
        1
    }
}

/// Enabled ordinary MixMap format-1 write, before packing the signed halfword.
/// 82928860..82928894 and 829289CC..829289E8 differ below -4800: an existing
/// modulation node resets to zero, while the direct path clamps to -4800.
/// The inputs are evaluated record +08, signed write adjustment and optional
/// modulation-node +14. This does not evaluate or infer their producers.
pub fn format1_pitch(record: i32, adjustment: i16, modulation: Option<i32>) -> i16 {
    let total = record
        .wrapping_add(modulation.unwrap_or(0))
        .wrapping_add(i32::from(adjustment));
    if total > 2400 {
        2400
    } else if total < -4800 {
        if modulation.is_some() { 0 } else { -4800 }
    } else {
        total as i16
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionPitch {
    pub controller_pitch: i32,
    pub combined_pitch: i32,
    pub ratio: f32,
}

/// A null controller returns zero directly. An existing controller holding
/// zero cents instead yields unity (4096) before the authored pitch is applied.
pub fn collision_pitch(
    authored_pitch: i32,
    controller: Option<&[u32]>,
    slot: usize,
    tables: &PitchTables,
) -> Result<CollisionPitch, VoiceError> {
    let controller_pitch = if controller.is_some() {
        let cents = controller_word(controller, slot)? as i16;
        truncate(tables.ratio(cents) * 4096.)?
    } else {
        0
    };
    let combined_pitch =
        truncate(controller_pitch as f32 * (authored_pitch as f32 * (1. / 4096.)))?;
    Ok(CollisionPitch {
        controller_pitch,
        combined_pitch,
        ratio: combined_pitch as f32 * (1. / 4096.),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables() -> PitchTables {
        // Synthetic distinguishable entries test routing and arithmetic; owned
        // values are checked separately against the actual instruction block.
        let mut table = PitchTables {
            semitones: [1.; 12],
            cents: [1.; 100],
        };
        table.semitones[1] = 1.25;
        table.cents[1] = 1.0625;
        table
    }
    #[test]
    fn format1_preserves_distinct_lower_bounds_and_word_overflow() {
        for value in [-4800, -1, 0, 2400] {
            assert_eq!(format1_pitch(value, 0, None), value as i16);
            assert_eq!(format1_pitch(value, 0, Some(0)), value as i16);
        }
        assert_eq!(format1_pitch(-4801, 0, None), -4800);
        assert_eq!(format1_pitch(-4801, 0, Some(0)), 0);
        assert_eq!(format1_pitch(2401, 0, Some(0)), 2400);
        assert_eq!(format1_pitch(-4900, 100, Some(1)), -4799);
        assert_eq!(format1_pitch(i32::MAX, 1, None), -4800);
        assert_eq!(format1_pitch(i32::MAX, 0, Some(1)), 0);
        assert_eq!(format1_pitch(i32::MIN, -1, None), 2400);
    }
    #[test]
    fn cents_and_octave_boundaries_keep_negative_arithmetic() {
        let table = tables();
        assert_eq!(table.ratio(0), 1.);
        assert_eq!(table.ratio(1200), 2.);
        assert_eq!(table.ratio(-1200), 0.5);
        assert_eq!(table.ratio(1301), 1.0625 * 1.25 * 2.);
        assert_eq!(table.ratio(-1301), ((1.0f32 / 1.25) / 2.) * (1. / 1.0625));
        assert!(table.ratio(i16::MIN).is_finite());
    }
    #[test]
    fn signed_halfwords_null_owner_and_authored_pitch_remain_distinct() {
        let table = tables();
        assert_eq!(
            collision_pitch(3796, Some(&[0]), 1, &table).unwrap().ratio,
            3796. / 4096.
        );
        assert_eq!(collision_pitch(4096, None, 1, &table).unwrap().ratio, 0.);
        assert_eq!(
            collision_pitch(4096, Some(&[(-1200i16 as u16 as u32) << 16]), 1, &table)
                .unwrap()
                .ratio,
            0.5
        );
        assert_eq!(
            collision_pitch(4096, Some(&[]), 1, &table),
            Err(VoiceError::IncompleteController)
        );
        assert_eq!(
            collision_pitch(4096, Some(&[32767]), 0, &table),
            Err(VoiceError::IntegerConversion)
        );
    }
    #[test]
    fn special_class_and_absent_material_choose_different_slots() {
        assert_eq!(pitch_slot(0x61, 5), 1);
        assert_eq!(pitch_slot(0x61, 9), 22);
        assert_eq!(pitch_slot(0x8f, 9), 1);
    }
    #[test]
    fn table_reader_rejects_truncation_extra_words_and_invalid_entries() {
        let bytes = |count| {
            (0..count)
                .flat_map(|_| 1.0f32.to_be_bytes())
                .collect::<Vec<_>>()
        };
        assert!(PitchTables::from_be_bytes(&bytes(12), &bytes(100)).is_some());
        for n in [11, 13] {
            assert!(PitchTables::from_be_bytes(&bytes(n), &bytes(100)).is_none());
        }
        for value in [0., -1., f32::NAN, f32::INFINITY] {
            let mut cents = bytes(100);
            cents[..4].copy_from_slice(&f32::to_be_bytes(value));
            assert!(PitchTables::from_be_bytes(&bytes(12), &cents).is_none());
        }
    }
}
