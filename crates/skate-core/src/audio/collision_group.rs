//! Collision group allocation, base-disc 824DF400..824DF4B0.
//! Entries must retain original linked-list creation order. Timestamps are
//! publication words from *(830734B4)+10, not elapsed host seconds.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    /// Original byte +34; any nonzero value means active.
    pub active: u8,
    /// Original unsigned word +40.
    pub timestamp: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub index: usize,
    /// Run the original group reset before accepting another record.
    pub reset: bool,
}

/// Select without changing state. This preserves the null result when all
/// active timestamps equal FFFFFFFF, and the first entry on other ties.
/// Group reset, record ownership and completion are separate lifecycle work.
pub fn select(groups: &[Group]) -> Option<Selection> {
    if let Some(index) = groups.iter().position(|group| group.active == 0) {
        return Some(Selection {
            index,
            reset: false,
        });
    }
    let mut timestamp = u32::MAX;
    let mut selected = None;
    for (index, group) in groups.iter().enumerate() {
        if group.timestamp < timestamp {
            timestamp = group.timestamp;
            selected = Some(Selection { index, reset: true });
        }
    }
    selected
}

/// Collision voice input publication, 824BFCA0..824BFE04. Channel booleans
/// represent the two nonnull playback pointers, not selected material sides.
/// Categories are the queued record's signed words +08/+0C. The callback
/// preserves native write ordering; a null controller binding skips all writes.
pub fn publish_inputs(
    bound: bool,
    active: u8,
    channels: [bool; 2],
    categories: [i32; 2],
    mut write: impl FnMut(usize, u32),
) {
    if !bound {
        return;
    }
    write(0, 0);
    write(1, 0);
    if active == 0 || !channels.into_iter().any(|channel| channel) {
        return;
    }
    write(0, 32767);
    let [first, second] = categories;
    let category = if first == 3 {
        second
    } else if second == 3 || first > second {
        first
    } else {
        second
    };
    write(
        1,
        match category {
            1 => 20000,
            2 => 32767,
            _ => 10000,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_requires_a_live_channel_and_preserves_clear_then_set_order() {
        let mut writes = Vec::new();
        publish_inputs(true, 255, [false, true], [3, 2], |slot, word| {
            writes.push((slot, word))
        });
        assert_eq!(writes, [(0, 0), (1, 0), (0, 32767), (1, 32767)]);
        writes.clear();
        publish_inputs(true, 1, [false, false], [2, 2], |slot, word| {
            writes.push((slot, word))
        });
        assert_eq!(writes, [(0, 0), (1, 0)]);
        writes.clear();
        publish_inputs(false, 1, [true, true], [2, 2], |slot, word| {
            writes.push((slot, word))
        });
        assert!(writes.is_empty());
    }

    #[test]
    fn publication_excludes_category_three_then_compares_signed_words() {
        for (categories, expected) in [
            ([2, 3], 32767),
            ([3, 1], 20000),
            ([3, 3], 10000),
            ([i32::MIN, 1], 20000),
            ([2, i32::MAX], 10000),
        ] {
            let mut words = [99; 2];
            publish_inputs(true, 1, [true, false], categories, |slot, word| {
                words[slot] = word
            });
            assert_eq!(words, [32767, expected]);
        }
    }

    #[test]
    fn inactive_entry_precedes_age_and_does_not_require_reset() {
        let groups = [
            Group {
                active: 255,
                timestamp: 0,
            },
            Group {
                active: 0,
                timestamp: u32::MAX,
            },
            Group {
                active: 0,
                timestamp: 0,
            },
        ];
        assert_eq!(
            select(&groups),
            Some(Selection {
                index: 1,
                reset: false
            })
        );
    }

    #[test]
    fn active_selection_preserves_unsigned_order_ties_and_maximum_boundary() {
        let mut groups = [Group {
            active: 1,
            timestamp: u32::MAX,
        }; 3];
        assert_eq!(select(&[]), None);
        assert_eq!(select(&groups), None);
        groups[1].timestamp = 0x80000000;
        groups[2].timestamp = 0x80000000;
        assert_eq!(
            select(&groups),
            Some(Selection {
                index: 1,
                reset: true
            })
        );
        groups[2].timestamp = 7;
        assert_eq!(
            select(&groups),
            Some(Selection {
                index: 2,
                reset: true
            })
        );
    }
}
