//! Observation accumulation 82D83820 and bin selection 82D84414.
//! Authored data is scoring_wipeouts/default, not an audio gain table.

#[derive(Clone, Copy, Debug)]
pub struct Neighbor {
    /// Native -1 means absent; other indices address the 25 scoring entries.
    pub entry: Option<usize>,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Record {
    pub neighbors: [Neighbor; 4],
    /// In authored order. Zero thresholds are skipped, not removed or sorted.
    pub thresholds: [f32; 6],
}

#[derive(Default, Clone, Debug)]
pub struct Accumulators {
    /// Entry +28 retains the peak until the original worker reset.
    pub peaks: [f32; 25],
    /// Entry +2C decays once per active worker classification update.
    pub accumulated: [f32; 25],
}

impl Accumulators {
    /// The caller supplies the original scaled observation or authored graph
    /// contribution. The root is not gated here; recursive links are gated.
    pub fn add(&mut self, table: &[Record; 25], entry: usize, value: f32) {
        self.propagate(table, entry, entry, value);
    }

    fn propagate(&mut self, table: &[Record; 25], entry: usize, source: usize, value: f32) {
        let accumulated = self.accumulated[entry] + value;
        self.accumulated[entry] = accumulated;
        let peak = self.peaks[entry];
        // Native fsel tests the subtraction, including its unordered behavior.
        self.peaks[entry] = if peak - accumulated >= 0.0 {
            peak
        } else {
            accumulated
        };
        for neighbor in table[entry].neighbors {
            if let Some(next) = neighbor.entry {
                if next != source {
                    let contribution = neighbor.weight * value;
                    if contribution > f32::from_bits(0x3DCC_CCCD) {
                        self.propagate(table, next, entry, contribution);
                    }
                }
            }
        }
    }

    /// The caller must copy the previous classification before refreshing
    /// observations, then use this result as the current classification.
    pub fn classify(&mut self, table: &[Record; 25]) -> [i32; 25] {
        std::array::from_fn(|entry| {
            let bin = (0..6).rev().find(|&bin| {
                let threshold = table[entry].thresholds[bin];
                threshold > 0.0 && self.peaks[entry] > threshold
            });
            self.accumulated[entry] *= f32::from_bits(0x3F4C_CCCD);
            bin.map_or(-1, |bin| bin as i32)
        })
    }
}
