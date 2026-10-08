//! Recovered audio behavior, independent of playback devices and host cadence.
pub mod body_impact;
pub mod board_contact;
pub mod deck_impact;
pub mod envelope;
pub mod grind_start;
pub mod hom;
pub mod clock;
pub mod curve;
pub mod collision_group;
pub mod collision_mix;
pub mod collision_position;
pub mod material;
pub mod modulation;
pub mod output;
pub mod pitch;
pub mod pan;
pub mod scalar;
pub mod voice;

/// Ordered linear-control product shared by declarations and envelopes.
/// Only a present array reaches this helper; null-array behavior is stage-specific.
fn control_weight(controls: &[u16]) -> Option<i32> {
    let mut weight: i32 = 32767;
    for &control in &controls[..controls.len() & 255] {
        if control > 32767 {
            return None;
        }
        weight = weight.wrapping_mul(i32::from(control)) >> 15;
    }
    Some(weight)
}

/// Aligned, bounded big-endian word access for owned MixMap sections.
fn read_word(data: &[u8], offset: usize) -> Option<u32> {
    if offset % 4 != 0 {
        return None;
    }
    Some(u32::from_be_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Effective family counts for sections with cross-family reference expansion.
fn read_groups(data: &[u8], counts: &[u8]) -> Result<(Vec<u32>, Vec<u8>), sum::SumError> {
    use sum::SumError;
    let word = |offset| read_word(data, offset).ok_or(SumError::InvalidProgram);
    let families = word(4)? as usize;
    if families > 256 || counts.len() != families || counts.iter().any(|n| *n > 32) {
        return Err(SumError::InvalidInstances);
    }
    let directory = word(8)? as usize;
    let mut groups = Vec::with_capacity(families);
    let mut effective = counts.to_vec();
    for (family, count) in effective.iter_mut().enumerate() {
        let offset = directory
            .checked_add(family * 4)
            .ok_or(SumError::InvalidProgram)?;
        let group = word(offset)?;
        if group == u32::MAX {
            *count = 0;
        }
        groups.push(group);
    }
    Ok((groups, effective))
}

/// Decode owned integer lookup tables once, before evaluating audio controls.
fn read_level_table<const N: usize>(bytes: &[u8], maximum: i32) -> Option<[i32; N]> {
    if bytes.len() != N * 4 {
        return None;
    }
    let values =
        std::array::from_fn(|i| i32::from_be_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()));
    values
        .iter()
        .all(|v| (0..=maximum).contains(v))
        .then_some(values)
}
pub mod input;
pub mod level_program;
pub mod main_objects;
pub mod master;
pub mod scalar_program;
pub mod sum;
