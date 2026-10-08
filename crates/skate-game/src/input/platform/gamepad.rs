//! Repacks a standard gamepad into the XInput state the TU3 converter reads
//! (`skate_core::input::xbox`). Values are not filtered here; the converter's
//! own deadzone stays authoritative.
use skate_core::input::xbox::XboxState;

/// One controller's readings, by physical position on an Xbox-layout pad.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PadValues {
    /// Up, down, left, right.
    pub(crate) dpad: [bool; 4],
    pub(crate) start: bool,
    pub(crate) back: bool,
    /// Left and right stick clicks.
    pub(crate) thumbs: [bool; 2],
    pub(crate) shoulders: [bool; 2],
    /// A, B, X, Y.
    pub(crate) faces: [bool; 4],
    /// Left and right, 0 to 1.
    pub(crate) triggers: [f32; 2],
    /// x and y from -1 to 1, with +y up as in XInput.
    pub(crate) left: [f32; 2],
    pub(crate) right: [f32; 2],
}

// XINPUT_GAMEPAD_* button bits.
const DPAD: [u16; 4] = [0x0001, 0x0002, 0x0004, 0x0008];
const START: u16 = 0x0010;
const BACK: u16 = 0x0020;
const THUMBS: [u16; 2] = [0x0040, 0x0080];
const SHOULDERS: [u16; 2] = [0x0100, 0x0200];
const FACES: [u16; 4] = [0x1000, 0x2000, 0x4000, 0x8000];

pub(crate) fn pack(values: &PadValues) -> XboxState {
    let held = values
        .dpad
        .iter()
        .zip(DPAD)
        .chain([(&values.start, START), (&values.back, BACK)])
        .chain(values.thumbs.iter().zip(THUMBS))
        .chain(values.shoulders.iter().zip(SHOULDERS))
        .chain(values.faces.iter().zip(FACES));
    XboxState {
        buttons: held
            .filter(|(pressed, _)| **pressed)
            .fold(0, |buttons, (_, bit)| buttons | bit),
        // Float-to-integer casts saturate and map NaN to 0.
        triggers: values.triggers.map(|v| (v.clamp(0., 1.) * 255.).round() as u8),
        left: values.left.map(axis),
        right: values.right.map(axis),
    }
}

fn axis(value: f32) -> i16 {
    (value.clamp(-1., 1.) * 32767.).round() as i16
}

/// Keeps each still-connected controller in its slot and gives new ones the
/// lowest free slot, as XInput numbers pads. Controllers beyond four wait.
pub(crate) fn assign<T: PartialEq + Clone>(slots: &mut [Option<T>; 4], connected: &[T]) {
    for slot in slots.iter_mut() {
        if slot.as_ref().is_some_and(|held| !connected.contains(held)) {
            *slot = None;
        }
    }
    for controller in connected {
        if slots.iter().flatten().any(|held| held == controller) {
            continue;
        }
        if let Some(free) = slots.iter_mut().find(|slot| slot.is_none()) {
            *free = Some(controller.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_control_lands_on_its_xinput_bit() {
        let cases: [(fn(&mut PadValues), u16); 14] = [
            (|v| v.dpad[0] = true, 0x0001),
            (|v| v.dpad[1] = true, 0x0002),
            (|v| v.dpad[2] = true, 0x0004),
            (|v| v.dpad[3] = true, 0x0008),
            (|v| v.start = true, 0x0010),
            (|v| v.back = true, 0x0020),
            (|v| v.thumbs[0] = true, 0x0040),
            (|v| v.thumbs[1] = true, 0x0080),
            (|v| v.shoulders[0] = true, 0x0100),
            (|v| v.shoulders[1] = true, 0x0200),
            (|v| v.faces[0] = true, 0x1000),
            (|v| v.faces[1] = true, 0x2000),
            (|v| v.faces[2] = true, 0x4000),
            (|v| v.faces[3] = true, 0x8000),
        ];
        for (press, bit) in cases {
            let mut values = PadValues::default();
            press(&mut values);
            assert_eq!(pack(&values).buttons, bit);
        }
    }

    #[test]
    fn analog_values_use_xinput_ranges() {
        let state = pack(&PadValues {
            triggers: [1., 0.5],
            left: [1., -1.],
            right: [0.25, f32::NAN],
            ..Default::default()
        });
        assert_eq!(state.triggers, [255, 128]);
        assert_eq!(state.left, [32767, -32767]);
        assert_eq!(state.right, [8192, 0]);
        let clamped = pack(&PadValues {
            triggers: [2., -1.],
            left: [3., -3.],
            ..Default::default()
        });
        assert_eq!(clamped.triggers, [255, 0]);
        assert_eq!(clamped.left, [32767, -32767]);
    }

    #[test]
    fn packed_states_ignore_changes_below_xinput_precision() {
        let a = PadValues { left: [0.5, 0.], ..Default::default() };
        let b = PadValues { left: [0.500_001, 0.], ..Default::default() };
        assert!(pack(&a) == pack(&b));
        assert!(pack(&a) != pack(&PadValues { faces: [true, false, false, false], ..a }));
    }

    #[test]
    fn controllers_keep_their_slots_and_refill_the_lowest_free_one() {
        let mut slots = [None; 4];
        assign(&mut slots, &[10, 11, 12]);
        assert_eq!(slots, [Some(10), Some(11), Some(12), None]);
        assign(&mut slots, &[10, 12]);
        assert_eq!(slots, [Some(10), None, Some(12), None]);
        assign(&mut slots, &[10, 12, 13, 14, 15]);
        assert_eq!(slots, [Some(10), Some(13), Some(12), Some(14)]);
    }
}
