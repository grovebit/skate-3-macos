//! +08 modulation record loading/instantiation, base-disc 8292B680..8292B924.
use super::{Mode, VolumeNode};
use crate::audio::{input::WordReference, read_groups, read_word, sum::SumError};
use std::sync::Arc;

pub struct BoundVolume {
    pub family: u8,
    pub instance: u8,
    pub index: u8,
    /// Canonical kind-3 unpacked input owner; not the collision output owner.
    pub owner: u32,
    pub node: VolumeNode,
}
impl BoundVolume {
    pub fn id(&self) -> u32 {
        0x80000000
            | (u32::from(self.family) << 16)
            | (u32::from(self.instance) << 11)
            | u32::from(self.index)
    }
}

/// Load selected dependency records in family/instance/file order. Counts
/// come from the original factory registry; no host player count is inferred.
pub fn load(
    data: &[u8],
    counts: &[u8],
    selected: &[(u8, u8)],
) -> Result<Vec<BoundVolume>, SumError> {
    let (groups, effective) = read_groups(data, counts)?;
    let word = |offset| read_word(data, offset).ok_or(SumError::InvalidProgram);
    let mut selection = std::collections::HashSet::new();
    for &(family, index) in selected {
        if usize::from(family) >= groups.len() || !selection.insert((family, index)) {
            return Err(SumError::InvalidProgram);
        }
    }
    let mut found = std::collections::HashSet::new();
    let mut result = Vec::new();
    for (family, (&group, &count)) in groups.iter().zip(&effective).enumerate() {
        if group == u32::MAX {
            continue;
        }
        let group = group as usize;
        let relative = word(group.checked_add(8).ok_or(SumError::InvalidProgram)?)? as i32;
        if relative < 0 {
            continue;
        }
        let section = group
            .checked_add(relative as usize)
            .ok_or(SumError::InvalidProgram)?;
        let size = word(section)? as i32;
        if !(0..=256).contains(&size) {
            return Err(SumError::InvalidProgram);
        }
        let mut cursor = section.checked_add(16).ok_or(SumError::InvalidProgram)?;
        let mut records = Vec::new();
        for index in 0..size {
            let header = word(cursor)?;
            let modes = ((header >> 24) & 15) as usize;
            let end = cursor
                .checked_add(4 + modes * 24)
                .ok_or(SumError::InvalidProgram)?;
            if end > data.len() {
                return Err(SumError::InvalidProgram);
            }
            if selection.contains(&(family as u8, index as u8)) {
                let modes = (0..modes)
                    .map(|i| {
                        let offset = cursor + 4 + i * 24;
                        let word_mode = word(offset)?;
                        let curves = word(offset + 4)?;
                        let mut bounds = [[0; 2]; 4];
                        for (j, pair) in bounds.iter_mut().enumerate() {
                            let raw = word(offset + 8 + j * 4)?;
                            *pair = [(raw & 32767) as u16, ((raw >> 16) & 32767) as u16];
                        }
                        Ok(Mode {
                            word: word_mode,
                            curves,
                            bounds,
                        })
                    })
                    .collect::<Result<Vec<_>, SumError>>()?;
                let modes: Arc<[Mode]> = modes.into();
                VolumeNode::new(modes.clone()).ok_or(SumError::InvalidProgram)?;
                records.push((index as u8, header, modes));
                found.insert((family as u8, index as u8));
            }
            cursor = end;
        }
        for instance in 0..count {
            for &(index, header, ref modes) in &records {
                // 8292B8D4..B8E8 ORs authored bits into the kind-3 instance ID.
                let source = 0x60000000 | (header & 0x1fffffff) | (u32::from(instance) << 11);
                result.push(BoundVolume {
                    family: family as u8,
                    instance,
                    index,
                    owner: WordReference::decode(source)
                        .ok_or(SumError::InvalidProgram)?
                        .owner,
                    node: VolumeNode::new(modes.clone()).ok_or(SumError::InvalidProgram)?,
                });
            }
        }
    }
    if found != selection {
        return Err(SumError::InvalidProgram);
    }
    Ok(result)
}
