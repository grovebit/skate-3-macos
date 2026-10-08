//! Selected authored +18 envelope records, base-disc 8292B148..8292B678.
//! Only gated type 1, bit 9 clear is executable here. Selection comes from
//! the requested graph dependencies, never from guessed inactive host states.
use super::{GatedEnvelope, LevelControl, Phase, State};
use crate::audio::{curve::CurveTable, scalar::ScalarTables, sum::SumError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeError {
    Program(SumError),
    InvalidSelection,
    UnsupportedRecord,
    InvalidInputs,
}
impl From<SumError> for EnvelopeError {
    fn from(value: SumError) -> Self {
        Self::Program(value)
    }
}

pub struct BoundEnvelope {
    pub family: u8,
    pub instance: u8,
    pub index: u8,
    pub descriptor: u32,
    /// Source construction ORs in instance bits (8292B610..8292B624).
    pub source: u32,
    /// None retains a null multiplier array. Some(empty) is a present array
    /// after a cross-family expansion with no instances (829267B0..829268AC).
    pub multipliers: Option<Vec<u32>>,
    envelope: GatedEnvelope,
    controls: LevelControl,
    log: i32,
}

impl BoundEnvelope {
    /// Canonical resolver identity; kind-5 lookup ignores selector/flag bits.
    pub fn id(&self) -> u32 {
        0xa0000000
            | (u32::from(self.family) << 16)
            | (u32::from(self.instance) << 11)
            | u32::from(self.index)
    }

    pub fn state(&self) -> State {
        self.envelope.state()
    }
    pub fn log(&self) -> i32 {
        self.log
    }

    /// Resolved source is a full word; any nonzero value triggers the state
    /// machine. Multipliers are linear fields, not scalar declaration levels.
    /// Caller supplies original evaluator milliseconds and binding order.
    pub fn advance(
        &mut self,
        delta: f32,
        source: u32,
        multipliers: Option<&[u16]>,
        curves: &CurveTable,
        tables: &ScalarTables,
    ) -> Result<i32, EnvelopeError> {
        if self.multipliers.as_ref().map(Vec::len) != multipliers.map(<[u16]>::len) {
            return Err(EnvelopeError::InvalidInputs);
        }
        // Commit together only after all supported diagnostic inputs succeed.
        let mut next = self.envelope;
        let idle = next.state().phase == Phase::Idle && source == 0;
        let linear = next
            .advance(delta, source != 0, curves)
            .ok_or(EnvelopeError::InvalidInputs)?;
        let log = if idle {
            // Outer fast path 82928B54..82928BA4 skips log/multiplier stages.
            0
        } else {
            self.controls
                .evaluate(linear as u16, multipliers, tables)
                .ok_or(EnvelopeError::InvalidInputs)?
        };
        self.envelope = next;
        self.log = log;
        Ok(log)
    }
}

/// Authored source and multiplier references of every +18 record, keyed by
/// (family, file index), using the loader's traversal stride. Used only to
/// discover a dependency cone; it neither validates nor binds record types.
pub(crate) fn authored_references(
    data: &[u8],
    counts: &[u8],
) -> Result<std::collections::HashMap<(u8, u8), Vec<u32>>, EnvelopeError> {
    let (groups, _) = crate::audio::read_groups(data, counts)?;
    let word = |offset: usize| {
        crate::audio::read_word(data, offset)
            .ok_or(EnvelopeError::Program(SumError::InvalidProgram))
    };
    let mut result = std::collections::HashMap::new();
    for (family, &group) in groups.iter().enumerate() {
        if group == u32::MAX {
            continue;
        }
        let group = group as usize;
        let relative = word(group.checked_add(24).ok_or(SumError::InvalidProgram)?)? as i32;
        if relative < 0 {
            continue;
        }
        let section = group
            .checked_add(relative as usize)
            .ok_or(SumError::InvalidProgram)?;
        let size = word(section)? as i32;
        if !(0..=256).contains(&size) {
            return Err(SumError::InvalidProgram.into());
        }
        let mut cursor = section.checked_add(16).ok_or(SumError::InvalidProgram)?;
        for index in 0..size {
            let level_word = word(cursor.checked_add(4).ok_or(SumError::InvalidProgram)?)?;
            let stored = ((level_word >> 16) & 15) as usize;
            let end = cursor
                .checked_add((6 + stored) * 4)
                .ok_or(SumError::InvalidProgram)?;
            if end > data.len() {
                return Err(SumError::InvalidProgram.into());
            }
            let mut references = vec![word(cursor + 8)?];
            for i in 0..stored {
                references.push(word(cursor + 24 + i * 4)?);
            }
            result.insert((family as u8, index as u8), references);
            cursor = end;
        }
    }
    Ok(result)
}

/// Bind selected (family, file index) records for every explicit instance,
/// preserving family/instance/file order and zero construction state. The
/// parser traverses unselected records without claiming their runtime behavior.
pub fn load(
    data: &[u8],
    counts: &[u8],
    selected: &[(u8, u8)],
    tables: &ScalarTables,
) -> Result<Vec<BoundEnvelope>, EnvelopeError> {
    let (groups, effective) = crate::audio::read_groups(data, counts)?;
    let word = |offset| {
        crate::audio::read_word(data, offset)
            .ok_or(EnvelopeError::Program(SumError::InvalidProgram))
    };
    let mut selection = std::collections::HashSet::new();
    for &(family, index) in selected {
        if usize::from(family) >= groups.len() || !selection.insert((family, index)) {
            return Err(EnvelopeError::InvalidSelection);
        }
    }
    let mut found = std::collections::HashSet::new();
    let mut result = Vec::new();
    for (family, (&group, &count)) in groups.iter().zip(&effective).enumerate() {
        if group == u32::MAX {
            continue;
        }
        let group = group as usize;
        let relative = word(group.checked_add(24).ok_or(SumError::InvalidProgram)?)? as i32;
        if relative < 0 {
            continue;
        }
        let section = group
            .checked_add(relative as usize)
            .ok_or(SumError::InvalidProgram)?;
        let size = word(section)? as i32;
        if !(0..=256).contains(&size) {
            return Err(SumError::InvalidProgram.into());
        }
        let mut cursor = section.checked_add(16).ok_or(SumError::InvalidProgram)?;
        let mut records = Vec::new();
        for index in 0..size {
            let descriptor = word(cursor)?;
            let level_word = word(cursor.checked_add(4).ok_or(SumError::InvalidProgram)?)?;
            // Traversal uses the low four bits of LHZ(record+4), not the
            // binder's low five or the rewritten high-byte runtime count.
            let stored = ((level_word >> 16) & 15) as usize;
            let length = (6 + stored) * 4;
            let end = cursor.checked_add(length).ok_or(SumError::InvalidProgram)?;
            if end > data.len() {
                return Err(SumError::InvalidProgram.into());
            }
            if selection.contains(&(family as u8, index as u8)) {
                if (descriptor >> 24) & 15 != 1
                    || descriptor & 0x300 != 0x100
                    || (level_word >> 16) & 31 != stored as u32
                {
                    return Err(EnvelopeError::UnsupportedRecord);
                }
                found.insert((family as u8, index as u8));
                let source = word(cursor + 8)?;
                let attack = word(cursor + 12)?;
                let release = word(cursor + 20)?;
                let gates = (0..stored)
                    .map(|i| word(cursor + 24 + i * 4))
                    .collect::<Result<Vec<_>, _>>()?;
                records.push((
                    index as u8,
                    descriptor,
                    level_word as i16,
                    source,
                    attack,
                    release,
                    gates,
                ));
            }
            cursor = end;
        }
        for instance in 0..count {
            for &(index, descriptor, authored, source, attack, release, ref gates) in &records {
                let multipliers = if gates.is_empty() {
                    None
                } else {
                    let mut expanded = Vec::new();
                    // 8292680C compares the reference family against the
                    // authored envelope descriptor's family, not its source.
                    let containing = ((descriptor >> 16) & 255) as u8;
                    for &gate in gates {
                        let target = ((gate >> 16) & 255) as u8;
                        if target == containing {
                            expanded.push(gate | (u32::from(instance) << 11));
                        } else {
                            let count = effective.get(usize::from(target)).copied().unwrap_or(0);
                            for instance in 0..count {
                                expanded.push(gate | (u32::from(instance) << 11));
                            }
                        }
                    }
                    Some(expanded)
                };
                result.push(BoundEnvelope {
                    family: family as u8,
                    instance,
                    index,
                    descriptor,
                    source: source | (u32::from(instance) << 11),
                    multipliers,
                    envelope: GatedEnvelope::from_authored(
                        attack,
                        release,
                        descriptor & 0x400 != 0,
                    ),
                    controls: LevelControl::from_authored(authored, tables),
                    log: 0,
                });
            }
        }
    }
    if found != selection {
        return Err(EnvelopeError::InvalidSelection);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables() -> (CurveTable, ScalarTables) {
        let bytes = |n, value: i32| (0..n).flat_map(|_| value.to_be_bytes()).collect::<Vec<_>>();
        (
            CurveTable::from_be_bytes(&bytes(513, 0)).unwrap(),
            ScalarTables::from_be_bytes(&bytes(512, 601), &bytes(602, 32730)).unwrap(),
        )
    }
    fn fixture() -> Vec<u8> {
        let mut words = vec![0_u32; 40];
        words[1] = 2;
        words[2] = 16;
        words[4] = 32;
        words[5] = 144;
        words[14] = 32; // First group +18 -> section 64.
        words[16] = 2;
        words[20] = 0xa1010500; // Descriptor family differs from containing family.
        words[21] = 0x0002d8f0;
        words[22] = 0x40013017;
        words[23] = 0x9000; // Authored zero -> one tick.
        words[24] = 5;
        words[25] = 0x9005;
        words[26] = 0x00011815;
        words[27] = 0x40003020;
        words[28] = 0xa1000301; // Bit 9 set remains unsupported.
        words[29] = 0;
        words[30] = 0x40000020;
        words[31] = 1;
        words[32] = 1;
        words[33] = 1;
        // Second group needs +18, so grow beyond its initial header.
        words.resize(43, 0);
        words[42] = u32::MAX;
        words.into_iter().flat_map(u32::to_be_bytes).collect()
    }

    #[test]
    fn binds_source_and_multipliers_with_original_or_and_descriptor_family() {
        let (_, tables) = tables();
        let bound = load(&fixture(), &[2, 1], &[(0, 0)], &tables).unwrap();
        assert_eq!(bound.len(), 2);
        assert_eq!((bound[0].id(), bound[1].id()), (0xa0000000, 0xa0000800));
        assert_eq!((bound[0].source, bound[1].source), (0x40013017, 0x40013817));
        assert_eq!(
            bound[0].multipliers.as_deref(),
            Some([0x00011815, 0x40003020, 0x40003820].as_slice())
        );
        assert_eq!(
            bound[1].multipliers.as_deref(),
            Some([0x00011815, 0x40003020, 0x40003820].as_slice())
        );
        assert_eq!(bound[0].state(), State::default());
        assert_eq!(bound[0].log(), 0);
    }

    #[test]
    fn idle_skips_weighting_and_invalid_active_inputs_leave_state_unchanged() {
        let (curves, tables) = tables();
        let mut bound = load(&fixture(), &[1, 1], &[(0, 0)], &tables).unwrap();
        let node = &mut bound[0];
        let gates = [32768, 32768];
        assert_eq!(node.advance(10.0, 0, Some(&gates), &curves, &tables), Ok(0));
        assert_eq!(node.state(), State::default());
        assert_eq!(
            node.advance(10.0, 0xffffffff, Some(&gates), &curves, &tables),
            Err(EnvelopeError::InvalidInputs)
        );
        assert_eq!(node.state(), State::default());
        assert_eq!(node.log(), 0);
        node.advance(
            f32::from_bits(0x41855557),
            0xffffffff,
            Some(&[32767; 2]),
            &curves,
            &tables,
        )
        .unwrap();
        assert_eq!(node.state().phase, Phase::Sustain);
        let state = node.state();
        assert_eq!(
            node.advance(f32::NAN, 1, Some(&[32767; 2]), &curves, &tables),
            Err(EnvelopeError::InvalidInputs)
        );
        assert_eq!(node.state(), state);
    }

    #[test]
    fn distinguishes_null_and_empty_bindings_and_rejects_bad_programs() {
        let (_, tables) = tables();
        let mut bytes = fixture();
        // Both gates now cross into an absent family, yielding a present empty array.
        bytes[104..108].copy_from_slice(&0x00020015_u32.to_be_bytes());
        bytes[108..112].copy_from_slice(&0x40020020_u32.to_be_bytes());
        let bound = load(&bytes, &[1, 0], &[(0, 0)], &tables).unwrap();
        assert_eq!(bound[0].multipliers.as_deref(), Some([].as_slice()));
        let mut no_gates = fixture();
        no_gates[112..116].copy_from_slice(&0xa1000101_u32.to_be_bytes());
        let bound = load(&no_gates, &[1, 0], &[(0, 1)], &tables).unwrap();
        assert_eq!(bound[0].multipliers, None);
        assert_eq!(
            load(&fixture(), &[1, 1], &[(0, 1)], &tables).err(),
            Some(EnvelopeError::UnsupportedRecord)
        );
        for selection in [vec![(0, 0), (0, 0)], vec![(0, 2)], vec![(2, 0)]] {
            assert_eq!(
                load(&fixture(), &[1, 1], &selection, &tables).err(),
                Some(EnvelopeError::InvalidSelection)
            );
        }
        for size in [0, 8, 16, 44, 60, 64, 80, 100, 111, 135] {
            assert!(load(&fixture()[..size], &[1, 1], &[(0, 0)], &tables).is_err());
        }
    }
}
