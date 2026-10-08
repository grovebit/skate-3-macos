//! Post-producer level stages of MixMap, base-disc 829283C4..82928520.
//! Scalar, modulation and envelope producers must finish before this phase.
use super::{
    output::{
        mxb::{self as output_mxb, OutputRecord},
        output_level,
    },
    scalar_program::ScalarProgram,
    sum::{SharedSum, SumError, mxb as sum_mxb},
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelError {
    Program(SumError),
    MissingReference(u32),
    UnsupportedSelfReference(u32),
    /// Kinds 6/7 (top bits C/E) resolve to one static word at 830282F4
    /// (829276C0); its producer is untraced.
    UnsupportedReference(u32),
    InvalidInputs,
}
impl From<SumError> for LevelError {
    fn from(e: SumError) -> Self {
        Self::Program(e)
    }
}

#[derive(Clone, Copy)]
enum Binding {
    Scalar(u8, u8, u8),
    Local(usize),
    External(usize),
}
struct SumNode {
    sum: SharedSum,
    inputs: HashMap<u32, Binding>,
}

pub struct BoundOutput {
    pub family: u8,
    pub instance: u8,
    pub record: OutputRecord,
    inputs: Vec<Binding>,
}

pub struct LevelProgram {
    sums: Vec<SumNode>,
    outputs: Vec<BoundOutput>,
    // Indexed snapshots updated after each node, preserving forward reads of
    // prior-phase levels. SharedSum owns its own null-array retention state.
    levels: Vec<i32>,
    external: Vec<u32>,
    scratch: Vec<i32>,
}

impl LevelProgram {
    pub fn from_mxb(
        data: &[u8],
        counts: &[u8],
        scalars: &ScalarProgram,
    ) -> Result<Self, LevelError> {
        let sums = sum_mxb::load(data, counts)?;
        let outputs = output_mxb::load(data, counts)?;
        let mut indices = HashMap::new();
        let mut sum_nodes = Vec::new();
        for instance in sums {
            for (index, sum) in instance.sums.into_iter().enumerate() {
                indices.insert(
                    (true, instance.family, instance.instance, index as u8),
                    sum_nodes.len(),
                );
                sum_nodes.push(SumNode {
                    sum,
                    inputs: HashMap::new(),
                });
            }
        }
        let sum_count = sum_nodes.len();
        let mut output_nodes = Vec::new();
        for instance in outputs {
            for (index, record) in instance.records.into_iter().enumerate() {
                indices.insert(
                    (false, instance.family, instance.instance, index as u8),
                    sum_count + output_nodes.len(),
                );
                output_nodes.push(BoundOutput {
                    family: instance.family,
                    instance: instance.instance,
                    record,
                    inputs: Vec::new(),
                });
            }
        }
        let mut external = Vec::new();
        let mut external_indices = HashMap::new();
        let mut bind = |id: u32, own: usize| -> Result<Binding, LevelError> {
            let family = (id >> 16) as u8;
            let instance = ((id >> 11) & 31) as u8;
            let index = id as u8;
            match id & 0xe0000000 {
                0 => {
                    scalars
                        .level(family, instance, index)
                        .ok_or(LevelError::MissingReference(id))?;
                    Ok(Binding::Scalar(family, instance, index))
                }
                0x20000000 => {
                    let local = *indices
                        .get(&(id & 0x10000000 != 0, family, instance, index))
                        .ok_or(LevelError::MissingReference(id))?;
                    // No owned sum/output record self-references. Native self
                    // reads observe a partially accumulated level, so reject
                    // this unsupported case rather than return a stale value.
                    if local == own {
                        return Err(LevelError::UnsupportedSelfReference(id));
                    }
                    Ok(Binding::Local(local))
                }
                0xc0000000 | 0xe0000000 => Err(LevelError::UnsupportedReference(id)),
                kind => {
                    // Word inputs share an owner slot. Modulation (r5=1: +0C)
                    // and envelope (+18) lookups ignore bits 8..10 and 24..28.
                    let id = match kind {
                        0x80000000 | 0xa0000000 => id & 0xe0fff8ff,
                        _ => super::input::WordReference::decode(id).map_or(id, |r| r.key()),
                    };
                    let index = *external_indices.entry(id).or_insert_with(|| {
                        let i = external.len();
                        external.push(id);
                        i
                    });
                    Ok(Binding::External(index))
                }
            }
        };
        for (own, node) in sum_nodes.iter_mut().enumerate() {
            for &id in node.sum.references().unwrap_or(&[]) {
                node.inputs.insert(id, bind(id, own)?);
            }
        }
        for (index, node) in output_nodes.iter_mut().enumerate() {
            node.inputs = node
                .record
                .bindings
                .levels
                .iter()
                .map(|id| bind(*id, sum_count + index))
                .collect::<Result<_, _>>()?;
        }
        let scratch_size = output_nodes
            .iter()
            .map(|node| node.inputs.len() & 255)
            .max()
            .unwrap_or(0);
        let mut levels = vec![0; sum_count];
        levels.resize(sum_count + output_nodes.len(), -10000); // 8292B0B4.
        Ok(Self {
            sums: sum_nodes,
            outputs: output_nodes,
            levels,
            external,
            scratch: Vec::with_capacity(scratch_size),
        })
    }

    /// Required non-scalar/non-sum producer levels. These are signed words,
    /// not normalized curve inputs. Missing producers are never zero-filled.
    /// Envelope keys select runtime +18 (log), not the linear +1C that scalar
    /// gates read; modulation keys select +0C. Keys keep the bound instance.
    pub fn external_inputs(&self) -> &[u32] {
        &self.external
    }
    pub fn outputs(&self) -> &[BoundOutput] {
        &self.outputs
    }
    /// Shared-sum accumulators in original family/instance/file order.
    pub fn sum_levels(&self) -> &[i32] {
        &self.levels[..self.sums.len()]
    }
    pub fn output_levels(&self) -> &[i32] {
        &self.levels[self.sums.len()..]
    }

    /// `enabled` captures each output owner's flag before the level phase.
    /// Packed writes follow this entire phase, never interleaved with it.
    pub fn advance(
        &mut self,
        scalars: &ScalarProgram,
        external: &[i32],
        enabled: &[bool],
    ) -> Result<(), LevelError> {
        if external.len() != self.external.len() || enabled.len() != self.outputs.len() {
            return Err(LevelError::InvalidInputs);
        }
        let read = |binding, levels: &[i32]| -> Option<i32> {
            match binding {
                Binding::Scalar(f, i, n) => scalars.level(f, i, n),
                Binding::Local(i) => Some(levels[i]),
                Binding::External(i) => Some(external[i]),
            }
        };
        // A different scalar layout is unsupported; validate before mutation.
        for binding in self
            .sums
            .iter()
            .flat_map(|n| n.inputs.values())
            .chain(self.outputs.iter().flat_map(|n| n.inputs.iter()))
        {
            if let Binding::Scalar(f, i, n) = *binding
                && scalars.level(f, i, n).is_none()
            {
                return Err(LevelError::InvalidInputs);
            }
        }
        for (i, node) in self.sums.iter_mut().enumerate() {
            self.levels[i] = node
                .sum
                .advance(|id| read(node.inputs[&id], &self.levels))?;
        }
        for (i, node) in self.outputs.iter().enumerate() {
            self.scratch.clear();
            if enabled[i] {
                for &binding in &node.inputs[..node.inputs.len() & 255] {
                    self.scratch.push(read(binding, &self.levels).unwrap());
                }
            }
            self.levels[self.sums.len() + i] =
                output_level(node.record.base, enabled[i], Some(&self.scratch));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{curve::CurveTable, scalar::ScalarTables};

    #[derive(Default)]
    struct Group {
        declarations: Vec<(u32, i16)>,
        sums: Vec<Vec<u32>>,
        outputs: Vec<(i16, u32, Vec<u32>)>,
    }

    /// Owned layout: group +04 declarations, +0C sums, +10 outputs, +14 maps.
    /// Other sections are absent. Sums use the -32768..32767 clamp word.
    fn program(groups: &[Option<Group>]) -> Vec<u8> {
        let mut w = vec![0, groups.len() as u32, 16, 0];
        w.resize(4 + groups.len(), u32::MAX);
        for (family, group) in groups.iter().enumerate() {
            let Some(group) = group else { continue };
            let start = w.len();
            w[4 + family] = 4 * start as u32;
            w.resize(start + 8, u32::MAX);
            let section = |w: &mut Vec<u32>, slot, header: [u32; 4]| {
                w[start + slot] = 4 * (w.len() - start) as u32;
                w.extend(header);
            };
            section(&mut w, 1, [group.declarations.len() as u32, 0, 0, 0]);
            for &(source, authored) in &group.declarations {
                w.extend([source, u32::from(authored as u16)]);
            }
            section(&mut w, 3, [group.sums.len() as u32, 0, 0, 0]);
            for references in &group.sums {
                w.extend([(references.len() as u32) << 16, 0x7fff8000]);
                w.extend(references);
            }
            let n = group.outputs.len() as u32;
            section(&mut w, 4, [n, n, 0, 0]);
            for (base, owner, arguments) in &group.outputs {
                w.extend([
                    (arguments.len() as u32) << 16,
                    u32::from(*base as u16) << 16,
                ]);
                w.push(*owner);
                w.extend(arguments);
            }
            section(&mut w, 5, [0xe0000001, 0x4812fbb4, 0xe0000001, 0x4812fbb4]);
        }
        w.into_iter().flat_map(u32::to_be_bytes).collect()
    }

    fn tables() -> (CurveTable, ScalarTables) {
        let bytes = |n, v: i32| (0..n).flat_map(|_| v.to_be_bytes()).collect::<Vec<_>>();
        (
            CurveTable::from_be_bytes(&bytes(513, 0)).unwrap(),
            ScalarTables::from_be_bytes(&bytes(512, 601), &bytes(602, 32730)).unwrap(),
        )
    }

    fn load(groups: &[Option<Group>], counts: &[u8]) -> (ScalarProgram, LevelProgram) {
        let bytes = program(groups);
        let scalars = ScalarProgram::from_mxb(&bytes, counts, &tables().1).unwrap();
        let levels = LevelProgram::from_mxb(&bytes, counts, &scalars).unwrap();
        (scalars, levels)
    }

    fn levels(program: &LevelProgram) -> (Vec<i32>, Vec<i32>) {
        (
            program.sum_levels().to_vec(),
            program.output_levels().to_vec(),
        )
    }

    #[test]
    fn forward_reads_see_the_previous_phase_and_backward_reads_the_current_one() {
        let main = Group {
            // Sum 0 reads sum 1 before it runs, like owned Main sums 2 and 3.
            sums: vec![
                vec![0x30000001, 0xb1000000],
                vec![0xb1000001],
                vec![0x30000001],
            ],
            outputs: vec![
                (
                    -5,
                    0x40000010,
                    vec![0x30000000, 0x30000002, 0xb1000000, 0x20000001],
                ),
                (7, 0x40000020, vec![0x20000000]),
            ],
            ..Group::default()
        };
        let (scalars, mut p) = load(&[Some(main)], &[1]);
        assert_eq!(p.external_inputs(), [0xa0000000, 0xa0000001]);
        // Construction zeroes sums (8292AEE8) and disables outputs (8292B0B4).
        assert_eq!(levels(&p), (vec![0; 3], vec![-10000; 2]));
        p.advance(&scalars, &[-100, -200], &[true, true]).unwrap();
        assert_eq!(levels(&p), (vec![-100, -200, -200], vec![-10405, -10398]));
        p.advance(&scalars, &[-1, -2], &[true, false]).unwrap();
        assert_eq!(levels(&p), (vec![-201, -2, -2], vec![-10607, -10000]));
        // A disabled earlier record publishes -10000 to later readers.
        p.advance(&scalars, &[0, 0], &[false, true]).unwrap();
        assert_eq!(levels(&p), (vec![-2, 0, 0], vec![-10000, -9993]));
    }

    #[test]
    fn scalar_and_sum_references_follow_their_separate_instance_bindings() {
        let main = Group {
            sums: vec![vec![0x00011800]],
            ..Group::default()
        };
        let player = Group {
            declarations: vec![(0x48010020, -602)],
            outputs: vec![(3, 0x40010030, vec![0x00011800, 0x30000000])],
            ..Group::default()
        };
        let (mut scalars, mut p) = load(&[Some(main), Some(player)], &[1, 2]);
        assert!(p.external_inputs().is_empty());
        let (curves, tables) = tables();
        // Selector 8 complements each instance's own controller word.
        assert_eq!(scalars.external_inputs(), [0x40010020, 0x40010820]);
        scalars.advance(&[0, 32767], &curves, &tables).unwrap();
        let (a, b) = (
            scalars.level(1, 0, 0).unwrap(),
            scalars.level(1, 1, 0).unwrap(),
        );
        assert_ne!(a, b);
        p.advance(&scalars, &[], &[true, true]).unwrap();
        // Cross-family scalar references expand to every Player instance;
        // same-family references bind only the containing instance.
        assert_eq!(
            levels(&p),
            (vec![a + b], vec![3 + a + a + b, 3 + b + a + b])
        );
    }

    #[test]
    fn resolver_identity_keys_external_inputs() {
        let main = Group {
            sums: vec![vec![
                0xb1000700, 0xa4000000, 0x4900002a, 0x4800002a, 0x80000101,
            ]],
            ..Group::default()
        };
        let (scalars, mut p) = load(&[Some(main)], &[1]);
        // Envelope bits 8..10/24..28 and curve flags do not select producers.
        assert_eq!(p.external_inputs(), [0xa0000000, 0x4000002a, 0x80000001]);
        p.advance(&scalars, &[-10, 20, -3], &[]).unwrap();
        assert_eq!(p.sum_levels(), [-10 - 10 + 20 + 20 - 3]);
    }

    #[test]
    fn invalid_frames_and_layouts_leave_all_levels_unchanged() {
        let main = Group {
            declarations: vec![(0x48000020, -602)],
            sums: vec![vec![0x00000000, 0xb1000000]],
            outputs: vec![(0, 0x40000010, vec![0x30000000])],
        };
        let (scalars, mut p) = load(&[Some(main)], &[1]);
        p.advance(&scalars, &[-7], &[true]).unwrap();
        let before = levels(&p);
        assert_eq!(
            p.advance(&scalars, &[], &[true]),
            Err(LevelError::InvalidInputs)
        );
        assert_eq!(
            p.advance(&scalars, &[-7], &[]),
            Err(LevelError::InvalidInputs)
        );
        let other = ScalarProgram::from_mxb(&program(&[None]), &[1], &tables().1).unwrap();
        assert_eq!(
            p.advance(&other, &[-7], &[true]),
            Err(LevelError::InvalidInputs)
        );
        assert_eq!(levels(&p), before);
    }

    #[test]
    fn rejects_self_missing_and_static_word_references() {
        let reject = |references: Vec<u32>| {
            let main = Group {
                sums: vec![references],
                ..Group::default()
            };
            let bytes = program(&[Some(main)]);
            let scalars = ScalarProgram::from_mxb(&bytes, &[1], &tables().1).unwrap();
            LevelProgram::from_mxb(&bytes, &[1], &scalars)
                .err()
                .unwrap()
        };
        assert_eq!(
            reject(vec![0x30000000]),
            LevelError::UnsupportedSelfReference(0x30000000)
        );
        for missing in [0x30000001, 0x20000000, 0x00000000] {
            assert_eq!(reject(vec![missing]), LevelError::MissingReference(missing));
        }
        for kind in [0xc0000000, 0xe1000005] {
            assert_eq!(reject(vec![kind]), LevelError::UnsupportedReference(kind));
        }
    }
}
