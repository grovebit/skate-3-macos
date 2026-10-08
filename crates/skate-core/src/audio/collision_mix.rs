//! Live volume and pitch controls of the original SFXObj_Collision voices.
//!
//! Each of the ten collision groups owns one packed controller buffer that
//! MixMap output records write (evaluator 82927E90, writer 82928530). The
//! voice update reads its material class's volume/pitch slots (12..21 and
//! 1/22, selectors 824BFF88/824C0158) from that buffer on every output phase (824C0334..824C03DC).
//! `CollisionMix` evaluates exactly the dependency cone of the collision
//! records that write those slots. The cone is found by following references
//! in the original program, then evaluated in the evaluator's stage order:
//! shared curves and declarations, modulation (82929F00), envelopes
//! (82928AF8), sums (829283C4), outputs (82928470) and packed writes
//! (82928530). Producers outside the cone are never evaluated. Every input
//! word comes from a caller-owned producer; nothing is defaulted here.
//! Base-disc default.xex SHA-256
//! 1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f.
use super::{
    curve::CurveTable,
    envelope::mxb::{self as envelope_mxb, BoundEnvelope},
    input::{WordInputs, WordReference},
    modulation::mxb::{self as modulation_mxb, BoundVolume},
    output::{
        ModulationOutput, WriteError,
        mxb::{self as output_mxb, OutputRecord},
        output_level, write_collision_controls,
    },
    scalar::ScalarTables,
    scalar_program::ScalarProgram,
    sum::{SharedSum, mxb as sum_mxb},
};
use std::collections::{BTreeSet, HashMap};

/// CSTATEMGR_Collision's family ID (registration 82FD0FBC).
pub const COLLISION_FAMILY: u8 = 3;
/// Packed slots read by the collision voice gain, selected by 824BFF88.
const VOLUME_SLOTS: std::ops::RangeInclusive<u32> = 12..=21;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MixError {
    /// The program is malformed or its cone could not be bound.
    Program,
    /// A reference kind, record type or mode outside the ported stages.
    Unsupported(u32),
    /// A required producer buffer was never attached.
    MissingInput(u32),
    /// A producer value outside the evaluator's verified domain.
    InvalidInput(u32),
    Write(WriteError),
}

/// Authored (family, file index) records reachable from the selected outputs.
#[derive(Debug, Default, PartialEq, Eq)]
struct Cone {
    declarations: BTreeSet<(u8, u8)>,
    envelopes: BTreeSet<(u8, u8)>,
    sums: BTreeSet<(u8, u8)>,
    modulations: BTreeSet<(u8, u8)>,
}

fn key(id: u32) -> (u8, u8, u8) {
    ((id >> 16) as u8, ((id >> 11) & 31) as u8, id as u8)
}

struct SumNode {
    family: u8,
    instance: u8,
    index: u8,
    sum: SharedSum,
}
struct OutputNode {
    instance: u8,
    record: OutputRecord,
}

pub struct CollisionMix {
    scalars: ScalarProgram,
    envelopes: Vec<BoundEnvelope>,
    envelope_index: HashMap<u32, usize>,
    modulations: Vec<BoundVolume>,
    modulation_index: HashMap<u32, usize>,
    sums: Vec<SumNode>,
    sum_index: HashMap<(u8, u8, u8), usize>,
    sum_levels: Vec<i32>,
    outputs: Vec<OutputNode>,
    output_levels: Vec<i32>,
    buffers: Vec<[u32; 16]>,
    required: BTreeSet<u32>,
    scalar_scratch: Vec<u16>,
    control_scratch: Vec<u16>,
    level_scratch: Vec<i32>,
    modulation_scratch: Vec<ModulationOutput>,
}

impl CollisionMix {
    /// `counts` are the original factory instance counts for every program
    /// family (Main 1, Player 2, Collision 10 for ordinary initialization).
    /// Families outside the cone may be given zero; a cone reference into a
    /// family with a zero count fails to bind instead of being dropped.
    pub fn from_mxb(data: &[u8], counts: &[u8], tables: &ScalarTables) -> Result<Self, MixError> {
        let program = |_| MixError::Program;
        let collision = output_mxb::load(data, counts)
            .map_err(program)?
            .into_iter()
            .filter(|instance| instance.family == COLLISION_FAMILY)
            .collect::<Vec<_>>();
        let first = collision.first().ok_or(MixError::Program)?;
        // Select writes read by voice gain/pitch, excluding record 2 (slots 2..11).
        let selected: Vec<usize> = first
            .records
            .iter()
            .enumerate()
            .filter(|(_, record)| {
                let count = (record.map_header & 31) as usize;
                record.writes[..count.min(record.writes.len())]
                    .iter()
                    .any(|write| {
                        let slot = (write >> 26) & 31;
                        VOLUME_SLOTS.contains(&slot) || matches!(slot, 1 | 22)
                    })
            })
            .map(|(index, _)| index)
            .collect();
        if selected.is_empty() {
            return Err(MixError::Program);
        }
        let mut roots = Vec::new();
        for &index in &selected {
            let record = &first.records[index];
            // Only volume and ordinary pitch formats are supported.
            if (record.map_header >> 24) & 15 > 1 {
                return Err(MixError::Unsupported(record.map_header));
            }
            roots.extend(&record.bindings.levels);
            roots.extend(&record.bindings.modulations);
        }
        let cone = discover(data, counts, &roots)?;

        let declarations: Vec<_> = cone.declarations.iter().copied().collect();
        let scalars = ScalarProgram::from_mxb_selected(data, counts, tables, &declarations)
            .map_err(|_| MixError::Program)?;
        let envelope_selection: Vec<_> = cone.envelopes.iter().copied().collect();
        let envelopes = envelope_mxb::load(data, counts, &envelope_selection, tables)
            .map_err(|_| MixError::Program)?;
        let modulation_selection: Vec<_> = cone.modulations.iter().copied().collect();
        let modulations =
            modulation_mxb::load(data, counts, &modulation_selection).map_err(program)?;
        for node in &modulations {
            // The evaluator mode producer is untraced; a single mode-zero
            // block makes mode changes irrelevant (82929F00..82929FC4).
            if node.node.modes() != 1 {
                return Err(MixError::Unsupported(node.id()));
            }
        }
        let mut sums = Vec::new();
        for instance in sum_mxb::load(data, counts).map_err(program)? {
            for (index, sum) in instance.sums.into_iter().enumerate() {
                if cone.sums.contains(&(instance.family, index as u8)) {
                    sums.push(SumNode {
                        family: instance.family,
                        instance: instance.instance,
                        index: index as u8,
                        sum,
                    });
                }
            }
        }
        let mut outputs = Vec::new();
        let mut buffers = Vec::new();
        for instance in collision {
            for (index, record) in instance.records.into_iter().enumerate() {
                if !selected.contains(&index) {
                    continue;
                }
                // Selected collision records share the instance's buffer.
                if record.buffer_index != 0 {
                    return Err(MixError::Unsupported(record.owner));
                }
                outputs.push(OutputNode {
                    instance: instance.instance,
                    record,
                });
            }
            // 82927AD0..82927B4C clears words 0..14, then stores the binding
            // helper's true result in word 15 (+3C): constructed enabled.
            let mut words = [0; 16];
            words[15] = 1;
            buffers.push(words);
        }
        let envelope_index = envelopes
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id(), i))
            .collect();
        let modulation_index = modulations
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id(), i))
            .collect();
        let sum_index = sums
            .iter()
            .enumerate()
            .map(|(i, s)| ((s.family, s.instance, s.index), i))
            .collect();
        let mut mix = Self {
            sum_levels: sums.iter().map(|s| s.sum.level()).collect(),
            output_levels: vec![-10000; outputs.len()], // 8292B0B4.
            scalars,
            envelopes,
            envelope_index,
            modulations,
            modulation_index,
            sums,
            sum_index,
            outputs,
            buffers,
            required: BTreeSet::new(),
            scalar_scratch: Vec::new(),
            control_scratch: Vec::new(),
            level_scratch: Vec::new(),
            modulation_scratch: Vec::new(),
        };
        mix.bind()?;
        Ok(mix)
    }

    /// Validate every reference once and record the producer owners.
    fn bind(&mut self) -> Result<(), MixError> {
        let mut required = BTreeSet::new();
        let mut word = |id: u32| match WordReference::decode(id) {
            Some(reference) => {
                required.insert(reference.owner);
                Ok(())
            }
            None => Err(MixError::Unsupported(id)),
        };
        for &id in self.scalars.external_inputs() {
            match id >> 29 {
                2 | 3 => word(id)?,
                4 if self.modulation_index.contains_key(&(id & 0xe0fff8ff)) => {}
                5 if self.envelope_index.contains_key(&(id & 0xe0fff8ff)) => {}
                _ => return Err(MixError::Unsupported(id)),
            }
        }
        for envelope in &self.envelopes {
            for &id in
                std::iter::once(&envelope.source).chain(envelope.multipliers.iter().flatten())
            {
                match id >> 29 {
                    0 => {
                        let (f, i, n) = key(id);
                        self.scalars.linear(f, i, n).ok_or(MixError::Program)?;
                    }
                    2 | 3 => word(id)?,
                    _ => return Err(MixError::Unsupported(id)),
                }
            }
        }
        for node in &self.modulations {
            required.insert(node.owner);
        }
        let levels = self
            .sums
            .iter()
            .flat_map(|s| s.sum.references().unwrap_or(&[]))
            .chain(self.outputs.iter().flat_map(|o| &o.record.bindings.levels));
        for &id in levels {
            self.level_index(id)?;
        }
        for output in &self.outputs {
            for &id in &output.record.bindings.modulations {
                self.modulation_index
                    .get(&(id & 0xe0fff8ff))
                    .ok_or(MixError::Program)?;
            }
        }
        self.required = required;
        Ok(())
    }

    fn level_index(&self, id: u32) -> Result<(), MixError> {
        let (f, i, n) = key(id);
        let found = match id >> 29 {
            0 => self.scalars.level(f, i, n).is_some(),
            1 if id & 0x10000000 != 0 => self.sum_index.contains_key(&(f, i, n)),
            4 => self.modulation_index.contains_key(&(id & 0xe0fff8ff)),
            5 => self.envelope_index.contains_key(&(id & 0xe0fff8ff)),
            _ => return Err(MixError::Unsupported(id)),
        };
        found.then_some(()).ok_or(MixError::Program)
    }

    /// Canonical owners (controller ID & E0FFFFF0) whose unpacked input
    /// buffers the caller must attach and publish before each phase.
    pub fn required_inputs(&self) -> impl Iterator<Item = u32> + '_ {
        self.required.iter().copied()
    }

    /// Number of collision-family instances (groups) evaluated.
    pub fn groups(&self) -> usize {
        self.buffers.len()
    }

    /// The packed output buffer the group's voices read (word 15 is the
    /// owner's enabled flag, +3C).
    pub fn words(&self, group: usize) -> Option<&[u32; 16]> {
        self.buffers.get(group)
    }

    /// Voice activation 824BFC58..824BFC74 stores 1 in output word +3C.
    pub fn activate(&mut self, group: usize) {
        if let Some(words) = self.buffers.get_mut(group) {
            words[15] = 1;
        }
    }

    /// Group reset 828B7E60 clears the whole output word +3C. It also zeroes
    /// the linked unpacked input buffers, which the caller owns.
    pub fn reset(&mut self, group: usize) {
        if let Some(words) = self.buffers.get_mut(group) {
            words[15] = 0;
        }
    }

    /// One MixMap phase. `elapsed` is the manager's accumulated MixMap-phase
    /// seconds; the evaluator stores it and multiplies it by binary32 1000
    /// (82251F78) for envelope milliseconds. Inputs are the words the
    /// producers published in preceding input phases. Every required owner
    /// is checked before any state changes; a producer value outside the
    /// verified domain can still fail mid-phase, after which the mix must
    /// not be used again.
    pub fn advance(
        &mut self,
        elapsed: f32,
        inputs: &mut WordInputs,
        curves: &CurveTable,
        tables: &ScalarTables,
    ) -> Result<(), MixError> {
        let milliseconds = elapsed * 1000.0;
        if !milliseconds.is_finite() || milliseconds < 0.0 {
            return Err(MixError::InvalidInput(0));
        }
        for &owner in &self.required {
            inputs.words(owner).ok_or(MixError::MissingInput(owner))?;
        }
        // Curve nodes then declarations. Kind-4/5 sources read the previous
        // phase: modulation and envelopes run after declarations.
        self.scalar_scratch.clear();
        for &id in self.scalars.external_inputs() {
            let value = match id >> 29 {
                2 | 3 => inputs.get(id).ok_or(MixError::MissingInput(id))?,
                4 => u32::from(
                    self.modulations[self.modulation_index[&(id & 0xe0fff8ff)]]
                        .node
                        .state()
                        .linear,
                ),
                _ => {
                    self.envelopes[self.envelope_index[&(id & 0xe0fff8ff)]]
                        .state()
                        .level as u32
                }
            };
            self.scalar_scratch.push(
                u16::try_from(value)
                    .ok()
                    .filter(|v| *v <= 32767)
                    .ok_or(MixError::InvalidInput(id))?,
            );
        }
        self.scalars
            .advance(&self.scalar_scratch, curves, tables)
            .map_err(|_| MixError::Program)?;
        for node in &mut self.modulations {
            let words = inputs
                .words_mut(node.owner)
                .ok_or(MixError::MissingInput(node.owner))?;
            node.node
                .advance(0, 0, words, curves, tables)
                .ok_or(MixError::InvalidInput(node.owner))?;
        }
        for index in 0..self.envelopes.len() {
            let envelope = &self.envelopes[index];
            let source = self.linear_or_word(envelope.source, inputs)?;
            self.control_scratch.clear();
            if let Some(multipliers) = &envelope.multipliers {
                for &id in multipliers {
                    let value = self.linear_or_word(id, inputs)?;
                    self.control_scratch.push(
                        u16::try_from(value)
                            .ok()
                            .filter(|v| *v <= 32767)
                            .ok_or(MixError::InvalidInput(id))?,
                    );
                }
            }
            let multipliers = envelope
                .multipliers
                .is_some()
                .then_some(self.control_scratch.as_slice());
            self.envelopes[index]
                .advance(milliseconds, source, multipliers, curves, tables)
                .map_err(|_| MixError::InvalidInput(source))?;
        }
        // Sums accumulate in place: a later sum read by an earlier one is
        // still its previous-phase level.
        for index in 0..self.sums.len() {
            let mut sum = std::mem::replace(&mut self.sums[index].sum, SharedSum::new(None, 0, 0));
            let result = sum.advance(|id| self.level(id));
            self.sums[index].sum = sum;
            self.sum_levels[index] = result.map_err(|_| MixError::Program)?;
        }
        for index in 0..self.outputs.len() {
            let output = &self.outputs[index];
            let enabled = self.buffers[usize::from(output.instance)][15] & 1 != 0;
            self.level_scratch.clear();
            if enabled {
                let levels = &output.record.bindings.levels;
                for &id in &levels[..levels.len() & 255] {
                    let level = self.level(id).ok_or(MixError::Program)?;
                    self.level_scratch.push(level);
                }
            }
            self.output_levels[index] =
                output_level(output.record.base, enabled, Some(&self.level_scratch));
        }
        for (index, output) in self.outputs.iter().enumerate() {
            self.modulation_scratch.clear();
            for id in &output.record.bindings.modulations {
                let node = &self.modulations[self.modulation_index[&(id & 0xe0fff8ff)]].node;
                let state = node.state();
                // Runtime +08 phase, +0C volume and +14 pitch (82928530).
                self.modulation_scratch.push(ModulationOutput {
                    raw: u32::from(state.phase),
                    volume: state.log,
                    pitch: node.pitch(),
                });
            }
            write_collision_controls(
                output.record.map_header,
                &output.record.writes,
                self.output_levels[index],
                &self.modulation_scratch,
                &mut self.buffers[usize::from(output.instance)],
                tables,
            )
            .map_err(MixError::Write)?;
        }
        Ok(())
    }

    /// Resolver r5=0: declarations give their shared curve's linear field,
    /// input words their full published value.
    fn linear_or_word(&self, id: u32, inputs: &WordInputs) -> Result<u32, MixError> {
        match id >> 29 {
            0 => {
                let (f, i, n) = key(id);
                Ok(u32::from(
                    self.scalars.linear(f, i, n).ok_or(MixError::Program)?,
                ))
            }
            _ => inputs.get(id).ok_or(MixError::MissingInput(id)),
        }
    }

    /// Resolver r5=1 levels (829276A0): declaration +08, sum +04,
    /// modulation +0C and envelope +18.
    fn level(&self, id: u32) -> Option<i32> {
        let (f, i, n) = key(id);
        match id >> 29 {
            0 => self.scalars.level(f, i, n),
            1 => self.sum_index.get(&(f, i, n)).map(|&s| self.sum_levels[s]),
            4 => self
                .modulation_index
                .get(&(id & 0xe0fff8ff))
                .map(|&m| self.modulations[m].node.state().log),
            5 => self
                .envelope_index
                .get(&(id & 0xe0fff8ff))
                .map(|&e| self.envelopes[e].log()),
            _ => None,
        }
    }
}

/// Follow references from the selected output records as the binders do:
/// declaration sources and gates, envelope sources and multipliers, and
/// sum references. Input words end a path. Output-to-output references and
/// the static word kinds 6/7 are outside the ported stages.
fn discover(data: &[u8], counts: &[u8], roots: &[u32]) -> Result<Cone, MixError> {
    let declarations = super::scalar_program::authored_declaration_references(data, counts)
        .map_err(|_| MixError::Program)?;
    let envelopes =
        envelope_mxb::authored_references(data, counts).map_err(|_| MixError::Program)?;
    let mut sums = HashMap::new();
    for instance in sum_mxb::load(data, counts).map_err(|_| MixError::Program)? {
        if instance.instance == 0 {
            for (index, sum) in instance.sums.iter().enumerate() {
                sums.insert(
                    (instance.family, index as u8),
                    sum.references().map(<[u32]>::to_vec).unwrap_or_default(),
                );
            }
        }
    }
    let mut cone = Cone::default();
    let mut pending = roots.to_vec();
    while let Some(id) = pending.pop() {
        let item = ((id >> 16) as u8, id as u8);
        let (set, references) = match id >> 29 {
            0 => (&mut cone.declarations, declarations.get(&item)),
            1 if id & 0x10000000 != 0 => (&mut cone.sums, sums.get(&item)),
            2 | 3 => continue,
            4 => {
                cone.modulations.insert(item);
                continue;
            }
            5 => (&mut cone.envelopes, envelopes.get(&item)),
            _ => return Err(MixError::Unsupported(id)),
        };
        let references = references.ok_or(MixError::Program)?;
        if set.insert(item) {
            pending.extend(references);
        }
    }
    Ok(cone)
}

#[cfg(test)]
mod tests;
