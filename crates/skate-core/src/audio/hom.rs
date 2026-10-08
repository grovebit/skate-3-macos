//! SFXObj_HOM publication, base-disc 824DC1F8..824DC498. Raw original
//! manager/context fields remain explicit; these are not host bail-state flags.

pub mod duration;
pub mod gate;
pub mod observation;

pub struct Inputs {
    pub manager_34b: u8,
    pub manager_34c: u8,
    /// Published float at context +29070, independent of host frame delta.
    pub timing_ratio: f32,
    /// Word +94 of the object at context +29070 +08; None means null pointer.
    pub secondary_flags: Option<u32>,
    pub manager_49c: i32,
    pub manager_36c: i32,
    pub manager_1cc: u8,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// Constructor 824DC0E8 clears both bytes.
    pub latch_1c: u8,
    pub latch_1d: u8,
}

/// Immediate producer of manager +34B/+34C, 824769F8..82476AAC.
/// The publication flags come from context +290B0 +20; their upstream
/// writer remains separate from host state. No threshold default is invented.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManagerFlags {
    pub active: u8,
    pub alternate: u8,
}

/// Fields of the selected publication actor's +3C object. Field ownership
/// and writers remain unported; names deliberately retain original offsets.
pub struct PublicationBody {
    pub flags_cc: u32,
    pub enabled_3937: u8,
    pub value_c8: f32,
}

/// Worker +7F4, cleared by 82D821D8. Counts worker updates, not seconds.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivityCounter {
    pub remaining_updates: u32,
}

pub struct PublicationActivity {
    pub transitions_d0: u32,
    pub value_c8: f32,
    pub enabled_3937: u8,
}

impl ActivityCounter {
    /// 82D8483C..82D8493C. Inputs are the original observation bins at
    /// entry +20/+24, with the previous bins copied before refresh.
    /// Their physics producers and worker cadence remain separate.
    pub fn advance(&mut self, current: &[i32; 25], previous: &[i32; 25]) -> PublicationActivity {
        let transitions = current
            .iter()
            .zip(previous)
            .filter(|(now, before)| **now >= 4 && **before < 4)
            .count() as u32;
        if transitions != 0 {
            self.remaining_updates = 30;
        } else if self.remaining_updates != 0 {
            self.remaining_updates -= 1;
        }
        PublicationActivity {
            transitions_d0: transitions,
            value_c8: if self.remaining_updates != 0 {
                1.0
            } else {
                0.0
            },
            enabled_3937: 1,
        }
    }
}

/// Bits contributed by 8277FC70..8277FCB8 after packing clears bits 25/26.
/// The caller must preserve the rest of the original publication word.
pub fn publication_bits(body: Option<&PublicationBody>) -> u32 {
    let Some(body) = body else { return 0 };
    if body.flags_cc & 0x30 == 0 || body.enabled_3937 == 0 {
        return 0;
    }
    0x04000000 | if body.value_c8 == 0.0 { 0x02000000 } else { 0 }
}

impl ManagerFlags {
    pub fn advance(
        &mut self,
        mode_354: i32,
        publication_flags: u32,
        timing_ratio: f32,
        threshold: f32,
    ) {
        self.active = if mode_354 == 1 && publication_flags & 0x04000000 != 0 {
            u8::from(!(timing_ratio > threshold && self.active == 0))
        } else {
            0
        };
        self.alternate = ((publication_flags >> 25) & 1) as u8;
    }
}

impl State {
    /// Original controller 400000C0 plus instance bits. Missing bindings
    /// suppress writes, while both state latches still follow native branches.
    pub fn publish(&mut self, inputs: &Inputs, bound: bool, mut write: impl FnMut(usize, u32)) {
        let mut publish = |slot, active| {
            if bound {
                write(slot, if active { 32767 } else { 0 });
            }
        };
        if inputs.manager_34b != 0 && inputs.timing_ratio < 1.0 {
            if inputs.manager_34c != 0 && self.latch_1c == 0 {
                publish(0, true);
                publish(3, false);
            } else {
                publish(0, false);
                publish(3, true);
                self.latch_1c = 1;
                self.latch_1d = 1;
            }
        } else {
            publish(0, false);
            publish(3, false);
            self.latch_1c = 0;
        }
        if inputs.secondary_flags.is_none_or(|flags| flags & 0x20 == 0) {
            self.latch_1d = 0;
        }
        publish(4, self.latch_1d != 0);
        publish(1, inputs.manager_49c == 1);
        publish(2, inputs.manager_36c == 31 && inputs.manager_1cc != 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_activation_retains_until_mode_or_publication_gate_clears() {
        let mut state = ManagerFlags::default();
        state.advance(1, 0x06000000, 1.1, 1.0);
        assert_eq!(
            state,
            ManagerFlags {
                active: 0,
                alternate: 1
            }
        );
        state.advance(1, 0x04000000, 1.0, 1.0);
        assert_eq!(state.active, 1);
        state.advance(1, 0x04000000, 1.1, 1.0);
        assert_eq!(state.active, 1);
        state.advance(0, 0x06000000, 0.5, 1.0);
        assert_eq!(
            state,
            ManagerFlags {
                active: 0,
                alternate: 1
            }
        );
    }

    #[test]
    fn latches_follow_publication_sequence_even_without_a_binding() {
        let mut state = State::default();
        let mut inputs = Inputs {
            manager_34b: 1,
            manager_34c: 0,
            timing_ratio: 0.5,
            secondary_flags: Some(0x20),
            manager_49c: 1,
            manager_36c: 31,
            manager_1cc: 1,
        };
        state.publish(&inputs, false, |_, _| panic!("null binding wrote a word"));
        assert_eq!(
            state,
            State {
                latch_1c: 1,
                latch_1d: 1
            }
        );
        inputs.manager_34c = 1;
        let mut writes = Vec::new();
        state.publish(&inputs, true, |slot, value| writes.push((slot, value)));
        assert_eq!(
            writes,
            [(0, 0), (3, 32767), (4, 32767), (1, 32767), (2, 32767)]
        );
        inputs.timing_ratio = 1.0;
        state.publish(&inputs, false, |_, _| unreachable!());
        assert_eq!(
            state,
            State {
                latch_1c: 0,
                latch_1d: 1
            }
        );
        inputs.secondary_flags = None;
        state.publish(&inputs, false, |_, _| unreachable!());
        assert_eq!(state, State::default());
    }
}
