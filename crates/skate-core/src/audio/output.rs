//! MixMap output-record accumulation, base-disc 82928484..82928508.
//! Runs after scalar/modulation/shared-sum evaluation and before packed writes.

use super::{
    pitch::format1_pitch,
    scalar::ScalarTables,
    sum::{SumError, expand_references},
};
pub mod mxb;

pub struct OutputBindings {
    pub levels: Vec<u32>,
    pub modulations: Vec<u32>,
}

impl OutputBindings {
    /// 8292BAA0..8292BCBC: authored modulation arguments form a prefix.
    /// They select the containing instance even for a different family;
    /// remaining level references use shared-sum expansion.
    pub fn new(
        arguments: &[u32],
        family: u8,
        instance: u8,
        counts: &[u8],
    ) -> Result<Self, SumError> {
        if instance >= 32 {
            return Err(SumError::InvalidInstance);
        }
        let modulation_count = arguments
            .iter()
            .filter(|id| **id & 0xe0000000 == 0x80000000)
            .count();
        let (modulations, levels) = arguments.split_at(modulation_count);
        // The native binder consumes a prefix rather than sorting. Reject
        // unsupported interleaving instead of silently changing its order.
        if modulations.iter().any(|id| *id & 0xe0000000 != 0x80000000) {
            return Err(SumError::InvalidProgram);
        }
        Ok(Self {
            levels: expand_references(levels, family, instance, counts)?,
            modulations: modulations
                .iter()
                .map(|id| (*id & 0xffff07ff) | (u32::from(instance) << 11))
                .collect(),
        })
    }
}

/// Original modulation runtime fields +08, +0C and +14. Producers must
/// supply these separately; pitch and raw controls are not volume levels.
#[derive(Clone, Copy)]
pub struct ModulationOutput {
    pub raw: u32,
    pub volume: i32,
    pub pitch: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    UnsupportedFormat(u8),
    IncompleteMap,
}

/// Original packed writer for collision formats 0 and 1 (82928530..82928AF0).
/// `words[15] & 1` is the owner's enabled flag. This neither attaches nor resets
/// the owner. Writes run in authored order, preserving each neighboring half.
pub fn write_collision_controls(
    header: u32,
    writes: &[u32],
    record: i32,
    modulations: &[ModulationOutput],
    words: &mut [u32; 16],
    tables: &ScalarTables,
) -> Result<(), WriteError> {
    let format = ((header >> 24) & 15) as u8;
    if format > 1 {
        return Err(WriteError::UnsupportedFormat(format));
    }
    let enabled = words[15] & 1 != 0;
    let count = if enabled { (header & 31) as usize } else { 1 };
    let writes = writes.get(..count).ok_or(WriteError::IncompleteMap)?;
    for &write in writes {
        let slot = ((write >> 26) & 31) as usize;
        let modulation_index = ((write >> 21) & 31) as usize;
        let modulation = if modulation_index < (modulations.len() & 31) {
            Some(&modulations[modulation_index])
        } else {
            None
        };
        let value = if !enabled {
            // Native disabled branch writes only the first map entry, even
            // with a zero execution count; it does not clear all slots.
            if format == 1 { 0 } else { (-10000_i16) as u16 }
        } else if let Some(modulation) = modulation.filter(|_| write & 0x80000000 != 0) {
            modulation.raw as u16
        } else if format == 0 {
            tables.format0_volume(record, write as i16, modulation.map(|m| m.volume))
        } else {
            format1_pitch(record, write as i16, modulation.map(|m| m.pitch)) as u16
        };
        let shift = 16 * (slot & 1);
        words[slot >> 1] = (words[slot >> 1] & !(0xffff << shift)) | (u32::from(value) << shift);
    }
    Ok(())
}

/// `base` is the signed high halfword of the authored value word. `inputs`
/// contains resolved scalar values, in binding order, with duplicates retained.
/// An enabled record always starts from its base, including null input arrays;
/// a disabled owner overwrites the record with -10000. Neither path retains the
/// previous level. Original evaluation uses only the low byte of input count.
pub fn output_level(base: i16, enabled: bool, inputs: Option<&[i32]>) -> i32 {
    if !enabled {
        return -10000;
    }
    let mut level = i32::from(base);
    if let Some(inputs) = inputs {
        for input in &inputs[..inputs.len() & 0xff] {
            level = level.wrapping_add(*input);
        }
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_binding_preserves_modulation_prefix_and_cross_family_expansion() {
        let bindings =
            OutputBindings::new(&[0x90003000, 0x30033000, 0xb100304c], 3, 2, &[2]).unwrap();
        assert_eq!(bindings.modulations, [0x90001000]);
        assert_eq!(bindings.levels, [0x30031000, 0xb100004c, 0xb100084c]);
        assert!(matches!(
            OutputBindings::new(&[0x30033000, 0x90030000], 3, 0, &[]),
            Err(SumError::InvalidProgram)
        ));
    }

    #[test]
    fn packed_collision_writes_preserve_neighbors_special_controls_and_disabled_slots() {
        let bytes = |n, value: i32| (0..n).flat_map(|_| value.to_be_bytes()).collect::<Vec<_>>();
        let tables = ScalarTables::from_be_bytes(&bytes(512, 0), &bytes(602, 32767)).unwrap();
        let mods = [ModulationOutput {
            raw: 0x1234abcd,
            volume: -602,
            pitch: -5000,
        }];
        let mut words = [0x5555aaaa; 16];
        words[15] |= 1;
        write_collision_controls(
            0xe0000003,
            &[0x48120000, 0x80000000, 0x4c330000],
            0,
            &mods,
            &mut words,
            &tables,
        )
        .unwrap();
        // Slot18 receives attenuation; slot19's out-of-range modulation
        // index selects the direct path. Special slot0 takes raw low bits.
        assert_eq!(words[9], 0x7fff3fff);
        assert_eq!(words[0], 0x5555abcd);
        write_collision_controls(0xe1000001, &[0x04010000], 0, &mods, &mut words, &tables).unwrap();
        assert_eq!(words[0], 0x0000abcd); // Modulated pitch below -4800 -> 0.
        words[15] &= !1;
        write_collision_controls(
            0xe0000002,
            &[0x48120000, 0x4c130000],
            0,
            &mods,
            &mut words,
            &tables,
        )
        .unwrap();
        assert_eq!(words[9], 0x7fffd8f0); // Only slot18 changes.
        let before = words;
        assert_eq!(
            write_collision_controls(0xe0000000, &[], 0, &[], &mut words, &tables),
            Err(WriteError::IncompleteMap)
        );
        assert_eq!(words, before);
    }

    #[test]
    fn enabled_null_and_empty_inputs_use_the_signed_base() {
        for base in [i16::MIN, -10000, -1, 0, 32767] {
            assert_eq!(output_level(base, true, None), i32::from(base));
            assert_eq!(output_level(base, true, Some(&[])), i32::from(base));
        }
        assert_eq!(output_level(42, false, Some(&[100])), -10000);
    }

    #[test]
    fn summation_preserves_duplicates_word_overflow_and_low_byte_count() {
        assert_eq!(output_level(-1, true, Some(&[100, 100, -50])), 149);
        assert_eq!(output_level(1, true, Some(&[i32::MAX])), i32::MIN);
        assert_eq!(output_level(-1, true, Some(&[i32::MIN])), i32::MAX);
        let values = [2; 257];
        for count in [255, 256, 257] {
            assert_eq!(
                output_level(3, true, Some(&values[..count])),
                3 + 2 * (count as i32 & 255)
            );
        }
    }

    #[test]
    fn output_feeds_pitch_with_different_direct_and_modulated_boundaries() {
        let level = output_level(0, true, Some(&[-3000, -2000]));
        assert_eq!(super::super::pitch::format1_pitch(level, 0, None), -4800);
        assert_eq!(super::super::pitch::format1_pitch(level, 0, Some(0)), 0);
        let disabled = output_level(0, false, Some(&[2400]));
        assert_eq!(disabled, -10000);
        // A disabled packed write has its own path; do not feed this level
        // through the enabled format conversion and call that owner behavior.
    }
}
