//! Ordinary collision voice controls, base-disc 824BFF88 and 824C0334..824C03DC.
//! Inputs must come from the queued record and its bound controller owner.

const NORMALIZATION: f32 = f32::from_bits(0x38000100);

/// Material +48 selects a packed volume-controller slot, not an event class.
pub fn volume_slot(material_class: i32) -> usize {
    match material_class {
        0..=5 => material_class as usize + 13,
        6 => 12,
        7..=9 => material_class as usize + 12,
        _ => 20,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceError {
    IncompleteController,
    /// PPC invalid/out-of-range integer conversion is outside this port's
    /// verified domain; do not silently substitute Rust's saturating cast.
    IntegerConversion,
}

/// 8249D138: numeric u32 words after big-endian decoding. A null controller
/// produces zero; a supplied but truncated capture is a different condition.
pub fn controller_level(words: Option<&[u32]>, slot: usize) -> Result<u16, VoiceError> {
    Ok(controller_word(words, slot)? & 0x7fff)
}

pub(super) fn controller_word(words: Option<&[u32]>, slot: usize) -> Result<u16, VoiceError> {
    let Some(words) = words else { return Ok(0) };
    let word = words
        .get(slot >> 1)
        .ok_or(VoiceError::IncompleteController)?;
    Ok((word >> (16 * (slot & 1))) as u16)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionGain {
    pub controller_level: u16,
    pub base_level: i32,
    pub after_controller: i32,
    pub after_material: i32,
    pub gain: f32,
}

/// Preserve all single-precision multiplies and both truncations. The material
/// resolver clamps the authored base before this consumer uses it. Neither
/// queued levels nor the resulting voice gain receive an extra host clamp.
pub fn collision_gain(
    queued_level: i32,
    authored_base_level: i32,
    controller: Option<&[u32]>,
    slot: usize,
) -> Result<CollisionGain, VoiceError> {
    let controller_level = controller_level(controller, slot)?;
    let base_level = authored_base_level.clamp(0, 32767);
    let after_controller =
        truncate((queued_level as f32 * NORMALIZATION) * controller_level as f32)?;
    let after_material = truncate(after_controller as f32 * (base_level as f32 * NORMALIZATION))?;
    Ok(CollisionGain {
        controller_level,
        base_level,
        after_controller,
        after_material,
        gain: after_material as f32 * NORMALIZATION,
    })
}

pub(super) fn truncate(value: f32) -> Result<i32, VoiceError> {
    if !(-2147483648.0..2147483648.0).contains(&value) {
        return Err(VoiceError::IntegerConversion);
    }
    Ok(value as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_words_preserve_halfword_order_and_ignore_flag_bits() {
        let words = [0xfedc9234, 0x80000001];
        assert_eq!(
            std::array::from_fn::<_, 4, _>(|slot| controller_level(Some(&words), slot).unwrap()),
            [0x1234, 0x7edc, 1, 0]
        );
        assert_eq!(controller_level(None, 18), Ok(0));
        assert_eq!(
            controller_level(Some(&words), 18),
            Err(VoiceError::IncompleteController)
        );
    }

    #[test]
    fn material_classes_select_original_slots_and_unsigned_fallback() {
        assert_eq!(
            std::array::from_fn::<_, 10, _>(|class| volume_slot(class as i32)),
            [13, 14, 15, 16, 17, 18, 12, 19, 20, 21]
        );
        for class in [-1, i32::MIN, 10, i32::MAX] {
            assert_eq!(volume_slot(class), 20);
        }
    }

    #[test]
    fn separate_truncations_preserve_sign_and_do_not_collapse_into_one_gain() {
        for sign in [-1, 1] {
            let result = collision_gain(sign * 10002, 18000, Some(&[12345]), 0).unwrap();
            assert_eq!(result.after_controller, sign * 3768);
            assert_eq!(result.after_material, sign * 2069);
            assert_eq!(result.gain, sign as f32 * 0.0631427988409996_f32);
        }
    }

    #[test]
    fn base_clamping_null_owner_and_conversion_boundaries() {
        for base in [32767, 40000] {
            assert_eq!(
                collision_gain(32767, base, Some(&[32767]), 0).unwrap().gain,
                1.
            );
        }
        for base in [-1, 0] {
            assert_eq!(
                collision_gain(32767, base, Some(&[32767]), 0).unwrap().gain,
                0.
            );
        }
        assert_eq!(collision_gain(32767, 32767, None, 18).unwrap().gain, 0.);
        assert_eq!(
            collision_gain(0, 32767, Some(&[32767]), 0).unwrap().gain,
            0.
        );
        assert_eq!(
            collision_gain(i32::MAX, 32767, Some(&[32767]), 0),
            Err(VoiceError::IntegerConversion)
        );
        assert_eq!(
            collision_gain(i32::MIN, 32767, Some(&[32767]), 0)
                .unwrap()
                .after_material,
            i32::MIN
        );
    }
}
