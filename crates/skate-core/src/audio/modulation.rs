//! Spatial volume fields of base-disc 82929F00..8292A450. Inputs are original
//! published distance/phase words, not inferred host camera or player values.
//! Pitch follows volume and mutates the shared input reset flags.
use super::{curve::CurveTable, scalar::ScalarTables};
pub mod mxb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode {
    pub word: u32,
    pub curves: u32,
    /// Low 15 bits followed by high 15 bits, 8292B7F0..8292B8D0.
    pub bounds: [[u16; 2]; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_and_far_branches_preserve_distinct_phase_semantics() {
        let curves = CurveTable::from_be_bytes(&vec![0; 513 * 4]).unwrap();
        let logs: Vec<u8> = (0_i32..512).flat_map(i32::to_be_bytes).collect();
        let tables = ScalarTables::from_be_bytes(&logs, &vec![0; 602 * 4]).unwrap();
        let mut node = VolumeNode::new(
            vec![Mode {
                word: 0,
                curves: 0x88880000,
                bounds: [[1, 50]; 4],
            }]
            .into(),
        )
        .unwrap();
        assert_eq!(node.state(), VolumeState::default());
        let mut inputs = [0; 16];
        inputs[1] = 51_f32.to_bits();
        inputs[3] = 49152;
        inputs[15] = 1;
        assert_eq!(
            node.advance(0, 0, &mut inputs, &curves, &tables),
            Some(VolumeState {
                phase: 49152,
                linear: 0,
                log: -10000,
            })
        );
        inputs[15] = 0;
        inputs[1] = f32::NAN.to_bits();
        inputs[3] = u32::MAX;
        assert_eq!(
            node.advance(0, 0, &mut inputs, &curves, &tables),
            Some(VolumeState {
                phase: 0,
                linear: 0,
                log: -10000,
            })
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeState {
    pub phase: u16,
    pub linear: u16,
    pub log: i32,
}
impl Default for VolumeState {
    /// Modulation construction at 8292B794..8292B7A4 differs from disabled
    /// evaluation: log starts at zero and linear starts at 32767.
    fn default() -> Self {
        Self {
            phase: 0,
            linear: 32767,
            log: 0,
        }
    }
}

pub struct VolumeNode {
    modes: std::sync::Arc<[Mode]>,
    selected: usize,
    // Runtime bounds are initialized from the first mode by 8292B7F0..B8D0.
    // The mode-pointer switch in 82929F00..9FC4 does not replace those fields.
    bounds: [[u16; 2]; 4],
    state: VolumeState,
    pitch: i32,
}

impl VolumeNode {
    /// Mode zero is required for the native mode-search fallback. Degenerate
    /// bounds/nonfinite producer values are outside this verified port domain.
    pub fn new(modes: std::sync::Arc<[Mode]>) -> Option<Self> {
        if modes.is_empty()
            || modes.len() > 15
            || !modes.iter().any(|mode| (mode.word >> 24) & 15 == 0)
            || modes[0]
                .bounds
                .iter()
                .any(|[low, high]| low >= high || *high > 32767)
        {
            return None;
        }
        Some(Self {
            bounds: modes[0].bounds,
            modes,
            selected: 0,
            state: VolumeState::default(),
            pitch: 0,
        })
    }

    pub fn state(&self) -> VolumeState {
        self.state
    }
    pub fn pitch(&self) -> i32 {
        self.pitch
    }

    pub fn selected_mode(&self) -> usize {
        self.selected
    }
    pub fn modes(&self) -> usize {
        self.modes.len()
    }

    /// The evaluator's prior/current mode words drive selection only when
    /// they differ. Pass the same pair to every node in that evaluation phase.
    /// Input word 15 bit 0 enables the node independently of output owners.
    pub fn advance(
        &mut self,
        previous_mode: u32,
        current_mode: u32,
        inputs: &mut [u32; 16],
        curves: &CurveTable,
        tables: &ScalarTables,
    ) -> Option<VolumeState> {
        let selected = if previous_mode != current_mode {
            self.modes
                .iter()
                .position(|m| (m.word >> 24) & 15 == current_mode)
                .or_else(|| self.modes.iter().position(|m| (m.word >> 24) & 15 == 0))?
        } else {
            self.selected
        };
        let mode = self.modes[selected];
        let (state, spatial) = self.evaluate(mode, inputs, curves, tables)?;
        if !spatial {
            self.pitch = 0; // Disabled or beyond both bounds: A180/A5DC.
        } else if mode.curves & 0xffff != 0 {
            // 8292A458..A5BC, after the volume result. Zero scale retains pitch.
            let alternate = (mode.word >> 12) & 15 == 1;
            let flag = if alternate { 0x80000000 } else { 0x40000000 };
            let target = if inputs[15] & flag != 0 {
                inputs[15] &= !flag;
                0
            } else {
                let scale = (mode.curves & 0xffff) as f32;
                let sum = f32::from_bits(inputs[if alternate { 13 } else { 14 }]) + scale;
                let denominator = if sum > 0.0 { sum } else { scale };
                tables.ratio_cents(scale / denominator)?
            };
            // +18/+1C distance bookkeeping has no consumer in this cone.
            let difference = target as f32 - self.pitch as f32;
            let step = difference * f32::from_bits(0xbe4ccccd); // 8208EA4C.
            if !step.is_finite() || !(-2147483648.0..2147483648.0).contains(&step) {
                return None;
            }
            self.pitch = self.pitch.wrapping_sub(step as i32);
        }
        self.selected = selected;
        self.state = state;
        Some(state)
    }

    fn evaluate(
        &self,
        mode: Mode,
        inputs: &[u32; 16],
        curves: &CurveTable,
        tables: &ScalarTables,
    ) -> Option<(VolumeState, bool)> {
        if inputs[15] & 1 == 0 {
            return Some((
                VolumeState {
                    phase: 0,
                    linear: 0,
                    log: -10000,
                },
                false,
            ));
        }
        let distance = match (mode.word >> 12) & 15 {
            0 => f32::from_bits(inputs[1]),
            1 => f32::from_bits(inputs[0]),
            _ => -1.0,
        };
        let phase = match (mode.word >> 8) & 15 {
            0 => inputs[3],
            1 => inputs[2],
            _ => 0,
        };
        if !distance.is_finite() || phase > 65535 {
            return None;
        }
        let quadrant = (phase >> 14) as usize;
        let remainder = (phase & 16383) as i32;
        let first = self.bounds[quadrant];
        let second = self.bounds[(quadrant + 1) & 3];
        if distance > f32::from(first[1]) && distance > f32::from(second[1]) {
            // Far branch retains the selected phase (8292A0BC, A170..A188).
            return Some((
                VolumeState {
                    phase: phase as u16,
                    linear: 0,
                    log: -10000,
                },
                false,
            ));
        }
        let selector = ((mode.curves >> [28, 16, 24, 20][quadrant]) & 15) as u8;
        let curve = |[low, high]: [u16; 2]| {
            let low = f32::from(low);
            let high = f32::from(high);
            let normalized = ((distance.clamp(low, high) - low) / (high - low)) * 32767.0;
            curves.evaluate(normalized as u16, selector).map(i32::from)
        };
        let first = curve(first)?;
        let second = if remainder == 0 {
            32767
        } else {
            curve(second)?
        };
        let weight = remainder * 2;
        let linear = ((first * (32767 - weight)) >> 15) + ((second * weight) >> 15);
        Some((
            VolumeState {
                phase: phase as u16,
                linear: linear as u16,
                log: tables.linear_to_log(linear as u16)?,
            },
            true,
        ))
    }
}
