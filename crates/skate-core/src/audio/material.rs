//! Counterpart class for collision event selection, base-disc
//! 82484474..82484490 and 82485750..82485884.

/// `surface_classes` contains authored audio mapping +1C for 94 surfaces
/// followed by the fallback entry. Entries are class indices 0, 1, or 2;
/// they are distinct from physical hardness and the voice volume-control class.
pub fn counterpart_class(material: i32, surface_classes: &[u8; 95]) -> u8 {
    match material {
        // The caller bypasses the helper for the absent-material sentinel.
        0x8f => 0,
        0x60 => 2,
        0x62 | 0x67 | 0x6b..=0x6d => 0,
        0x5f | 0x61 | 0x63..=0x66 | 0x68..=0x6a | 0x71 => 1,
        0..=0x5d => surface_classes[material as usize],
        _ => surface_classes[94],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_surfaces_fallback_and_absent_counterpart_differ() {
        let mut classes = [0; 95];
        classes[4] = 1;
        classes[93] = 2;
        classes[94] = 2;
        assert_eq!(counterpart_class(4, &classes), 1);
        assert_eq!(counterpart_class(93, &classes), 2);
        for material in [-1, 94, 0x6e, 0x6f, 0x70, 0x72, 0x90, i32::MAX] {
            assert_eq!(counterpart_class(material, &classes), 2);
        }
        assert_eq!(counterpart_class(0x8f, &classes), 0);
    }

    #[test]
    fn special_material_jump_table_preserves_every_entry() {
        let classes = [2; 95];
        // Independently transcribed jump targets at 82485784..824857CC.
        let expected = [1, 2, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 2, 2, 2, 1];
        for (offset, expected) in expected.into_iter().enumerate() {
            assert_eq!(counterpart_class(0x5f + offset as i32, &classes), expected);
        }
    }
}
