//! Bounded reader for the scalar declaration section (+04) of owned MixMap files.
use super::*;

impl ScalarProgram {
    /// Family counts must be supplied from the original factory registry. Zero
    /// explicitly omits a family; it is not an inferred inactive-player count.
    /// Other program sections remain outside this scalar runner.
    pub fn from_mxb(
        data: &[u8],
        counts: &[u8],
        tables: &ScalarTables,
    ) -> Result<Self, ProgramError> {
        Self::new(&decode(data, counts)?, tables)
    }

    /// Selected (family, file index) declarations for every configured
    /// instance. Unselected records are traversed but not constructed.
    pub fn from_mxb_selected(
        data: &[u8],
        counts: &[u8],
        tables: &ScalarTables,
        selected: &[(u8, u8)],
    ) -> Result<Self, ProgramError> {
        let instances = decode(data, counts)?;
        let selection: std::collections::HashSet<_> = selected.iter().copied().collect();
        if selection.len() != selected.len()
            || selection.iter().any(|&(family, index)| {
                !instances.iter().any(|instance| {
                    instance.family == family && usize::from(index) < instance.declarations.len()
                })
            })
        {
            return Err(ProgramError::InvalidProgram);
        }
        Self::build(&instances, tables, Some(&selection))
    }
}

/// Authored source and gate references of each declaration, keyed by
/// (family, file index), for families with a nonzero configured count.
pub(crate) fn authored_references(
    data: &[u8],
    counts: &[u8],
) -> Result<HashMap<(u8, u8), Vec<u32>>, ProgramError> {
    let mut result = HashMap::new();
    for instance in decode(data, counts)?
        .iter()
        .filter(|instance| instance.instance == 0)
    {
        for (index, spec) in instance.declarations.iter().enumerate() {
            let mut references = vec![spec.source];
            references.extend(&spec.gates);
            result.insert((instance.family, index as u8), references);
        }
    }
    Ok(result)
}

fn decode(data: &[u8], counts: &[u8]) -> Result<Vec<InstanceSpec>, ProgramError> {
    let word = |offset| super::super::read_word(data, offset).ok_or(ProgramError::InvalidProgram);
    let family_count = word(4)? as usize;
    if family_count > 256 || counts.len() != family_count || counts.iter().any(|n| *n > 32) {
        return Err(ProgramError::InvalidInstances);
    }
    let directory = word(8)? as usize;
    let mut instances = Vec::new();
    for (family, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let entry = directory
            .checked_add(family * 4)
            .ok_or(ProgramError::InvalidProgram)?;
        let group = word(entry)?;
        // 829256D8..82925700 leaves the effective count at zero and skips
        // the factory lookup when the program has no section for this family.
        if group == u32::MAX {
            continue;
        }
        let group = group as usize;
        let relative = word(group.checked_add(4).ok_or(ProgramError::InvalidProgram)?)? as i32;
        let mut declarations = Vec::new();
        if relative >= 0 {
            let section = group
                .checked_add(relative as usize)
                .ok_or(ProgramError::InvalidProgram)?;
            let size = word(section)? as i32;
            if !(0..=256).contains(&size) {
                return Err(ProgramError::InvalidProgram);
            }
            let mut cursor = section
                .checked_add(16)
                .ok_or(ProgramError::InvalidProgram)?;
            for _ in 0..size {
                let source = word(cursor)?;
                let descriptor = word(cursor.checked_add(4).ok_or(ProgramError::InvalidProgram)?)?;
                cursor = cursor.checked_add(8).ok_or(ProgramError::InvalidProgram)?;
                let mut gates = Vec::new();
                for _ in 0..((descriptor >> 16) & 31) {
                    gates.push(word(cursor)?);
                    cursor = cursor.checked_add(4).ok_or(ProgramError::InvalidProgram)?;
                }
                declarations.push(DeclarationSpec {
                    source,
                    authored: descriptor as i16,
                    gates,
                });
            }
        }
        for instance in 0..count {
            instances.push(InstanceSpec {
                family: family as u8,
                instance,
                declarations: declarations.clone(),
            });
        }
    }
    Ok(instances)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut words = vec![0; 24];
        words[1] = 1;
        words[2] = 16;
        words[4] = 32;
        words[9] = 32;
        words[16] = 1;
        words[20] = 0x49000020;
        words[21] = 0x0001fe0c;
        words[22] = 0x40000030;
        words.into_iter().flat_map(u32::to_be_bytes).collect()
    }
    #[test]
    fn reads_original_stride_signed_level_and_explicit_instance_counts() {
        let specs = decode(&fixture(), &[2]).unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[1].instance, 1);
        let d = &specs[1].declarations[0];
        assert_eq!(d.source, 0x49000020);
        assert_eq!(d.authored, -500);
        assert_eq!(d.gates, [0x40000030]);
    }
    #[test]
    fn absent_family_overrides_nonzero_registry_count() {
        let mut bytes = fixture();
        bytes[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode(&bytes, &[2]).unwrap().is_empty());
    }
    #[test]
    fn rejects_truncated_or_misaligned_sections_and_invalid_counts() {
        let bytes = fixture();
        for size in [0, 8, 16, 32, 64, 80, 88, 91] {
            assert!(decode(&bytes[..size], &[1]).is_err());
        }
        assert!(decode(&bytes, &[33]).is_err());
        assert!(decode(&bytes, &[]).is_err());
        let mut changed = bytes.clone();
        changed[8..12].copy_from_slice(&17u32.to_be_bytes());
        assert!(decode(&changed, &[1]).is_err());
        changed = bytes;
        changed[64..68].copy_from_slice(&257u32.to_be_bytes());
        assert!(decode(&changed, &[1]).is_err());
    }
}
