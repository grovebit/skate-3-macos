//! Authored +0C shared-sum section, base-disc 8292AD98..8292AF20.
use super::{SharedSum, SumError, expand_references};

pub struct SumInstance {
    pub family: u8,
    pub instance: u8,
    /// File order is the runtime index used by sum references.
    pub sums: Vec<SharedSum>,
}

/// Instantiate sums in family/instance/file order with the original zeroed
/// accumulator (8292AEE8). Factory counts are explicit; absent directory
/// entries override their count to zero, including during reference expansion.
/// This loads and binds IDs; their runtime producers are resolved at advance.
pub fn load(data: &[u8], counts: &[u8]) -> Result<Vec<SumInstance>, SumError> {
    let word = |offset| super::super::read_word(data, offset).ok_or(SumError::InvalidProgram);
    let (groups, effective_counts) = super::super::read_groups(data, counts)?;
    let mut result = Vec::new();
    for (family, (&group, &count)) in groups.iter().zip(&effective_counts).enumerate() {
        if count == 0 {
            continue;
        }
        let group = group as usize;
        let relative = word(group.checked_add(12).ok_or(SumError::InvalidProgram)?)? as i32;
        let mut records = Vec::new();
        if relative >= 0 {
            let section = group
                .checked_add(relative as usize)
                .ok_or(SumError::InvalidProgram)?;
            let size = word(section)? as i32;
            if !(0..=256).contains(&size) {
                return Err(SumError::InvalidProgram);
            }
            let mut cursor = section.checked_add(16).ok_or(SumError::InvalidProgram)?;
            for _ in 0..size {
                let descriptor = word(cursor)?;
                let clamp = word(cursor.checked_add(4).ok_or(SumError::InvalidProgram)?)?;
                cursor = cursor.checked_add(8).ok_or(SumError::InvalidProgram)?;
                let mut references = Vec::new();
                // LHZ of the first word followed by low-byte extraction.
                for _ in 0..((descriptor >> 16) & 255) {
                    references.push(word(cursor)?);
                    cursor = cursor.checked_add(4).ok_or(SumError::InvalidProgram)?;
                }
                records.push((clamp, references));
            }
        }
        for instance in 0..count {
            let sums = records
                .iter()
                .map(|(clamp, references)| {
                    let expanded =
                        expand_references(references, family as u8, instance, &effective_counts)?;
                    Ok(SharedSum::new(Some(expanded), *clamp, 0))
                })
                .collect::<Result<_, SumError>>()?;
            result.push(SumInstance {
                family: family as u8,
                instance,
                sums,
            });
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut words = vec![0_u32; 30];
        words[1] = 2;
        words[2] = 16;
        words[4] = 32;
        words[5] = u32::MAX;
        words[11] = 32;
        words[16] = 2;
        words[20] = 0xd0020000;
        words[21] = 0x0000d8f0;
        words[22] = 0x00003001;
        words[23] = 0x00013000;
        words[24] = 0xd0010001;
        words[25] = 0x0000d8f0;
        words[26] = 0x00003002;
        words.into_iter().flat_map(u32::to_be_bytes).collect()
    }
    #[test]
    fn loads_stride_instance_bindings_zero_state_and_absent_family_counts() {
        let mut instances = load(&fixture(), &[2, 3]).unwrap();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[1].family, 0);
        assert_eq!(instances[1].instance, 1);
        assert_eq!(instances[1].sums.len(), 2);
        assert_eq!(instances[0].sums[0].references(), Some([1].as_slice()));
        assert_eq!(instances[1].sums[0].references(), Some([0x801].as_slice()));
        assert_eq!(instances[1].sums[1].references(), Some([0x802].as_slice()));
        assert_eq!(instances[1].sums[0].level(), 0);
        assert_eq!(instances[1].sums[0].advance(|_| Some(-12000)), Ok(-10000));
    }
    #[test]
    fn rejects_truncation_misalignment_and_unsupported_counts() {
        let bytes = fixture();
        for size in [0, 8, 16, 20, 44, 64, 80, 88, 100, 107] {
            assert!(load(&bytes[..size], &[1, 0]).is_err());
        }
        assert!(load(&bytes, &[1]).is_err());
        assert!(load(&bytes, &[33, 0]).is_err());
        for (offset, value) in [(8, 17_u32), (44, 33), (64, 257)] {
            let mut changed = bytes.clone();
            changed[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            assert!(load(&changed, &[1, 0]).is_err());
        }
    }
}
