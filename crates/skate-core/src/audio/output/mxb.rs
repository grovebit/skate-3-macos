//! Output sections +10/+14, base-disc 8292AF28..8292B144 and 8292BAA0..8292BCBC.
use super::{OutputBindings, SumError};

pub struct OutputRecord {
    pub owner: u32,
    pub buffer_index: usize,
    pub base: i16,
    pub bindings: OutputBindings,
    pub map_header: u32,
    /// Stored map entries, including entries beyond the low-five-bit execution
    /// count. Traversal uses the full low byte; the writer chooses execution.
    pub writes: Vec<u32>,
}

pub struct OutputInstance {
    pub family: u8,
    pub instance: u8,
    pub buffer_count: usize,
    pub records: Vec<OutputRecord>,
}

struct AuthoredRecord {
    owner: u32,
    buffer_index: usize,
    base: i16,
    arguments: Vec<u32>,
    map_header: u32,
    writes: Vec<u32>,
}

/// Load and bind records in original construction order. Adjacent equal
/// authored owner IDs share a buffer; nonadjacent repeats allocate another.
/// This describes storage, not owner attachment or an assumed enabled state.
pub fn load(data: &[u8], counts: &[u8]) -> Result<Vec<OutputInstance>, SumError> {
    let word = |offset| super::super::read_word(data, offset).ok_or(SumError::InvalidProgram);
    let add = |base: usize, delta: usize| base.checked_add(delta).ok_or(SumError::InvalidProgram);
    let (groups, effective) = super::super::read_groups(data, counts)?;
    let mut result = Vec::new();
    for (family, (&group, &count)) in groups.iter().zip(&effective).enumerate() {
        if count == 0 {
            continue;
        }
        let group = group as usize;
        let relative = word(add(group, 16)?)? as i32;
        let mut authored = Vec::new();
        let mut buffer_count = 0;
        if relative >= 0 {
            let section = add(group, relative as usize)?;
            let size = word(section)? as i32;
            if !(0..=256).contains(&size) {
                return Err(SumError::InvalidProgram);
            }
            buffer_count = word(add(section, 4)?)? as usize;
            if size > 0 {
                let mapping = word(add(group, 20)?)? as i32;
                if mapping < 0 {
                    return Err(SumError::InvalidProgram);
                }
                let mut map = add(group, mapping as usize)?;
                let mut cursor = add(section, 16)?;
                let mut previous_owner = 0;
                let mut buffers_used = 0_usize;
                for _ in 0..size {
                    let descriptor = word(cursor)?;
                    let value = word(add(cursor, 4)?)?;
                    let owner = word(add(cursor, 8)?)?;
                    if owner != previous_owner {
                        buffers_used += 1;
                    }
                    // Native first-owner zero would point before the buffer
                    // allocation; reject it and out-of-range authored storage.
                    if buffers_used == 0 || buffers_used > buffer_count {
                        return Err(SumError::InvalidProgram);
                    }
                    previous_owner = owner;
                    cursor = add(cursor, 12)?;
                    let mut arguments = Vec::new();
                    for _ in 0..((descriptor >> 16) & 255) {
                        arguments.push(word(cursor)?);
                        cursor = add(cursor, 4)?;
                    }
                    let map_header = word(map)?;
                    map = add(map, 4)?;
                    let mut writes = Vec::new();
                    for _ in 0..(map_header & 255) {
                        writes.push(word(map)?);
                        map = add(map, 4)?;
                    }
                    authored.push(AuthoredRecord {
                        owner,
                        buffer_index: buffers_used - 1,
                        base: (value >> 16) as i16,
                        arguments,
                        map_header,
                        writes,
                    });
                }
            }
        }
        for instance in 0..count {
            let records = authored
                .iter()
                .map(|record| {
                    Ok(OutputRecord {
                        // 8292B0C4..B0D4 ORs instance bits; it does not clear them.
                        owner: record.owner | (u32::from(instance) << 11),
                        buffer_index: record.buffer_index,
                        base: record.base,
                        bindings: OutputBindings::new(
                            &record.arguments,
                            family as u8,
                            instance,
                            &effective,
                        )?,
                        map_header: record.map_header,
                        writes: record.writes.clone(),
                    })
                })
                .collect::<Result<_, SumError>>()?;
            result.push(OutputInstance {
                family: family as u8,
                instance,
                buffer_count,
                records,
            });
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut w = vec![0_u32; 52];
        w[1] = 2;
        w[2] = 16;
        w[4] = 32;
        w[5] = u32::MAX;
        w[12] = 32;
        w[13] = 128;
        w[16] = 3;
        w[17] = 2;
        w[20] = 0xc0020000;
        w[21] = 0xff9cd8f0;
        w[22] = 0x40001020;
        w[23] = 0x90000000;
        w[24] = 0x00010000;
        w[25] = 0xc0000001;
        w[26] = 0;
        w[27] = 0x40001020;
        w[28] = 0xc0000002;
        w[29] = 0;
        w[30] = 0x40000030;
        w[40] = 0xe0000001;
        w[41] = 0x4812fbb4;
        w[42] = 0xe1000001;
        w[43] = 0x04010000;
        w[44] = 0xe0000001;
        w[45] = 0x80000000;
        w.into_iter().flat_map(u32::to_be_bytes).collect()
    }
    #[test]
    fn pairs_maps_shares_adjacent_buffers_and_preserves_owner_instance_bits() {
        let loaded = load(&fixture(), &[2, 3]).unwrap();
        assert_eq!(loaded.len(), 2);
        let instance = &loaded[1];
        assert_eq!(instance.buffer_count, 2);
        assert_eq!(
            instance
                .records
                .iter()
                .map(|r| r.buffer_index)
                .collect::<Vec<_>>(),
            [0, 0, 1]
        );
        let r = &instance.records[0];
        assert_eq!(r.owner, 0x40001820);
        assert_eq!(r.base, -100);
        assert_eq!(r.bindings.modulations, [0x90000800]);
        assert!(r.bindings.levels.is_empty()); // Absent family overrides 3.
        assert_eq!(r.writes, [0x4812fbb4]);
        assert_eq!(instance.records[1].map_header, 0xe1000001);
        assert_eq!(instance.records[2].writes, [0x80000000]);
    }
    #[test]
    fn stored_write_count_and_nonadjacent_owner_repeats_are_preserved() {
        let mut bytes = fixture();
        bytes[68..72].copy_from_slice(&3_u32.to_be_bytes());
        bytes[108..112].copy_from_slice(&0x40000030_u32.to_be_bytes());
        bytes[120..124].copy_from_slice(&0x40001020_u32.to_be_bytes());
        let loaded = load(&bytes, &[1, 0]).unwrap();
        assert_eq!(
            loaded[0]
                .records
                .iter()
                .map(|r| r.buffer_index)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );

        bytes = fixture();
        bytes[64..68].copy_from_slice(&1_u32.to_be_bytes());
        bytes[160..164].copy_from_slice(&0xe0000020_u32.to_be_bytes());
        bytes.resize(164 + 32 * 4, 0);
        bytes[288..292].copy_from_slice(&0x4812fbb4_u32.to_be_bytes());
        let loaded = load(&bytes, &[1, 0]).unwrap();
        let r = &loaded[0].records[0];
        assert_eq!(r.map_header & 31, 0);
        assert_eq!(r.writes.len(), 32);
        assert_eq!(r.writes[31], 0x4812fbb4);
    }

    #[test]
    fn rejects_truncated_maps_missing_storage_and_misalignment() {
        let bytes = fixture();
        for size in [0, 16, 48, 64, 80, 124, 160, 183] {
            assert!(load(&bytes[..size], &[1, 0]).is_err());
        }
        for (offset, value) in [(52, u32::MAX), (68, 1), (88, 0), (48, 33), (64, 257)] {
            let mut changed = bytes.clone();
            changed[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            assert!(load(&changed, &[1, 0]).is_err());
        }
    }
}
