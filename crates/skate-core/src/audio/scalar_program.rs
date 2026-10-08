//! Bound scalar portion of the original MixMap evaluator. Curve inputs outside
//! scalar declarations remain explicit inputs from the other original producers.
//! This is not a complete MixMap VM or a host-time/controller adapter.
use super::{
    curve::{CurveTable, CurveValue},
    scalar::{ScalarDeclaration, ScalarTables},
};
use std::collections::HashMap;

#[derive(Clone)]
pub struct DeclarationSpec {
    pub source: u32,
    pub authored: i16,
    pub gates: Vec<u32>,
}
pub struct InstanceSpec {
    pub family: u8,
    pub instance: u8,
    pub declarations: Vec<DeclarationSpec>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramError {
    InvalidInstances,
    InvalidProgram,
    MissingScalar(u32),
    InvalidInputs,
}
#[derive(Clone, Copy)]
enum Binding {
    Curve(usize),
    External(usize),
    Zero,
}
struct CurveNode {
    id: u32,
    input: Binding,
    value: CurveValue,
}
struct Declaration {
    curve: usize,
    metadata: ScalarDeclaration,
    gates: Vec<Binding>,
    level: i32,
}

pub struct ScalarProgram {
    curves: Vec<CurveNode>,
    order: Vec<usize>,
    declarations: Vec<Declaration>,
    indices: HashMap<(u8, u8, u8), usize>,
    external: Vec<u32>,
    gate_scratch: Vec<u16>,
}
impl ScalarProgram {
    /// Instances must be supplied in original construction order and cover
    /// contiguous instance numbers starting at zero for each included family.
    /// Declaration arrays preserve the original file order (8292A6A8).
    pub fn new(instances: &[InstanceSpec], tables: &ScalarTables) -> Result<Self, ProgramError> {
        Self::build(instances, tables, None)
    }

    /// Construct only the selected (family, file index) declarations, keeping
    /// their original indices and relative construction order. Every scalar a
    /// selected declaration reads must also be selected; this evaluates a
    /// closed dependency cone, not a reordered or approximated program.
    pub(super) fn build(
        instances: &[InstanceSpec],
        tables: &ScalarTables,
        selected: Option<&std::collections::HashSet<(u8, u8)>>,
    ) -> Result<Self, ProgramError> {
        let mut counts = HashMap::<u8, u8>::new();
        for instance in instances {
            let count = counts.entry(instance.family).or_default();
            if instance.instance != *count || *count >= 32 || instance.declarations.len() > 256 {
                return Err(ProgramError::InvalidInstances);
            }
            *count += 1;
        }
        let mut curves = Vec::new();
        let mut nodes = HashMap::<u32, usize>::new();
        let mut declarations = Vec::new();
        let mut indices = HashMap::new();
        let mut gate_ids = Vec::new();
        for instance in instances {
            for (index, spec) in instance.declarations.iter().enumerate() {
                if spec.gates.len() > 31 {
                    return Err(ProgramError::InvalidProgram);
                }
                if selected.is_some_and(|s| !s.contains(&(instance.family, index as u8))) {
                    continue;
                }
                // 8292A738..A744: source identity clears authored instance bits.
                let id = (spec.source & 0xffff07ff) | (u32::from(instance.instance) << 11);
                let curve = *nodes.entry(id).or_insert_with(|| {
                    let index = curves.len();
                    curves.push(CurveNode {
                        id,
                        input: Binding::Zero,
                        value: CurveValue::default(),
                    });
                    index
                });
                indices.insert(
                    (instance.family, instance.instance, index as u8),
                    declarations.len(),
                );
                declarations.push(Declaration {
                    curve,
                    metadata: tables.declaration(spec.authored),
                    gates: Vec::new(),
                    level: 0,
                });
                let mut gates = Vec::new();
                // 829266B8 compares each gate family with the source ID family.
                // It ORs instance bits, unlike the source-identity operation.
                for &gate in &spec.gates {
                    let family = (gate >> 16) as u8;
                    if family == (spec.source >> 16) as u8 {
                        gates.push(gate | (u32::from(instance.instance) << 11));
                    } else {
                        for instance in 0..counts.get(&family).copied().unwrap_or(0) {
                            gates.push(gate | (u32::from(instance) << 11));
                        }
                    }
                }
                gate_ids.push(gates);
            }
        }
        let mut external = Vec::new();
        let mut external_indices = HashMap::new();
        let mut bind = |id: u32| -> Result<Binding, ProgramError> {
            if id & 0xe0000000 == 0 {
                let key = ((id >> 16) as u8, ((id >> 11) & 31) as u8, id as u8);
                let declaration = *indices.get(&key).ok_or(ProgramError::MissingScalar(id))?;
                Ok(Binding::Curve(declarations[declaration].curve))
            } else {
                // Controller input aliases discard curve flags and share the
                // same original owner/word (82927980..82927988).
                let id = super::input::WordReference::decode(id).map_or(id, |r| r.key());
                let index = *external_indices.entry(id).or_insert_with(|| {
                    let index = external.len();
                    external.push(id);
                    index
                });
                Ok(Binding::External(index))
            }
        };
        for curve in &mut curves {
            // 82927F0C guards invalid selectors before reading the input.
            if (curve.id >> 24) & 15 <= 9 {
                curve.input = bind(curve.id)?;
            }
        }
        let resolved: Vec<Vec<Binding>> = gate_ids
            .iter()
            .map(|ids| ids.iter().map(|id| bind(*id)).collect())
            .collect::<Result<_, _>>()?;
        let max_gates = resolved.iter().map(|g| g.len() & 255).max().unwrap_or(0);
        for (declaration, gates) in declarations.iter_mut().zip(resolved) {
            declaration.gates = gates;
        }
        let mut order: Vec<_> = (0..curves.len()).collect();
        // 829265C8 allocates shared input nodes in selector bins. Within each
        // bin, the first occurrence of the complete source ID defines order.
        order.sort_by_key(|i| (curves[*i].id >> 24) & 15);
        Ok(Self {
            curves,
            order,
            declarations,
            indices,
            external,
            gate_scratch: Vec::with_capacity(max_gates),
        })
    }

    /// Reference keys for unresolved non-scalar producers; word-input aliases
    /// share their canonical owner/slot key. Callers must
    /// supply their original linear values; absent data is never treated as zero.
    pub fn external_inputs(&self) -> &[u32] {
        &self.external
    }

    pub fn level(&self, family: u8, instance: u8, index: u8) -> Option<i32> {
        Some(self.declarations[*self.indices.get(&(family, instance, index))?].level)
    }

    /// Resolver r5=0 follows declaration -> shared curve -> linear +0C
    /// (82927744..82927758), rather than its logarithmic declaration level.
    pub fn linear(&self, family: u8, instance: u8, index: u8) -> Option<u16> {
        let declaration = &self.declarations[*self.indices.get(&(family, instance, index))?];
        Some(self.curves[declaration.curve].value.linear)
    }

    /// One original scalar evaluation phase. Shared curve nodes update in-place
    /// before declarations, so cross-node reads preserve original ordering and
    /// prior-phase values. This must not be replaced with a topological pass.
    pub fn advance(
        &mut self,
        inputs: &[u16],
        curves: &CurveTable,
        tables: &ScalarTables,
    ) -> Result<(), ProgramError> {
        if inputs.len() != self.external.len() || inputs.iter().any(|v| *v > 32767) {
            return Err(ProgramError::InvalidInputs);
        }
        for &index in &self.order {
            let source = Self::read(self.curves[index].input, &self.curves, inputs);
            let selector = ((self.curves[index].id >> 24) & 15) as u8;
            self.curves[index].value = curves
                .value(source, selector, tables)
                .expect("validated linear inputs");
        }
        for declaration in &mut self.declarations {
            self.gate_scratch.clear();
            for &gate in &declaration.gates[..declaration.gates.len() & 255] {
                self.gate_scratch
                    .push(Self::read(gate, &self.curves, inputs));
            }
            declaration.level = declaration
                .metadata
                .evaluate(
                    self.curves[declaration.curve].value.linear,
                    Some(&self.gate_scratch),
                    tables,
                )
                .expect("validated gate inputs")
                .level;
        }
        Ok(())
    }

    fn read(binding: Binding, curves: &[CurveNode], inputs: &[u16]) -> u16 {
        match binding {
            Binding::Curve(index) => curves[index].value.linear,
            Binding::External(index) => inputs[index],
            Binding::Zero => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables() -> (CurveTable, ScalarTables) {
        let bytes = |n, v: i32| (0..n).flat_map(|_| v.to_be_bytes()).collect::<Vec<_>>();
        (
            CurveTable::from_be_bytes(&bytes(513, 0)).unwrap(),
            ScalarTables::from_be_bytes(&bytes(512, 601), &bytes(602, 32730)).unwrap(),
        )
    }
    fn spec(source: u32) -> DeclarationSpec {
        DeclarationSpec {
            source,
            authored: 555,
            gates: vec![],
        }
    }
    #[test]
    fn scalar_levels_expand_into_sum_output_and_collision_volume() {
        use crate::audio::{
            output::{output_level, write_collision_controls},
            sum::{SharedSum, expand_references},
            voice,
        };
        // Synthetic tables isolate composition; owned-table parity is checked
        // by the local instruction replay, not inferred from this fixture.
        let (curves, tables) = tables();
        let instances: Vec<_> = (0..2)
            .map(|instance| InstanceSpec {
                family: 1,
                instance,
                declarations: vec![DeclarationSpec {
                    source: 0x49010020,
                    authored: -602,
                    gates: vec![],
                }],
            })
            .collect();
        let mut program = ScalarProgram::new(&instances, &tables).unwrap();
        program.advance(&[0, 0], &curves, &tables).unwrap();
        let references = expand_references(&[0x00013000], 3, 0, &[0, 2]).unwrap();
        let mut sum = SharedSum::new(Some(references), 0x0000d8f0, 0);
        let level = sum
            .advance(|id| program.level((id >> 16) as u8, ((id >> 11) & 31) as u8, id as u8))
            .unwrap();
        assert_eq!(level, -1206);
        let output = output_level(0, true, Some(&[level]));
        // Body material class 5 selects slot 18, the low half of word 9.
        let mut words = [0_u32; 16];
        words[15] = 1;
        write_collision_controls(0xe0000001, &[0x4812fbb4], output, &[], &mut words, &tables)
            .unwrap();
        assert_eq!(words[9], 4091);
        let gain =
            voice::collision_gain(20000, 32767, Some(&words), voice::volume_slot(5)).unwrap();
        assert_eq!(gain.after_controller, 2497);
        assert_eq!(gain.after_material, 2497);
    }
    #[test]
    fn master_publication_controls_feed_aliased_curve_inputs() {
        use crate::audio::{input::WordInputs, master::PublicationSpeed};
        let (curve, tables) = tables();
        let mut p = ScalarProgram::new(
            &[InstanceSpec {
                family: 0,
                instance: 0,
                declarations: vec![spec(0x49000025), spec(0x48000025)],
            }],
            &tables,
        )
        .unwrap();
        assert_eq!(p.external_inputs(), [0x40000025]);
        let mut inputs = WordInputs::default();
        PublicationSpeed::evaluate(Some(2.), false, 1.)
            .unwrap()
            .write(inputs.attach(0x40000020).unwrap());
        let frame: Vec<u16> = p
            .external_inputs()
            .iter()
            .map(|id| inputs.get(*id).unwrap().try_into().unwrap())
            .collect();
        p.advance(&frame, &curve, &tables).unwrap();
        assert_eq!(p.curves[0].value.linear, 8191);
        assert_eq!(p.curves[1].value.linear, 32767 - 8191);
    }
    #[test]
    fn shared_curve_state_preserves_selector_order_and_previous_phase_reads() {
        let (curve, tables) = tables();
        let mut p = ScalarProgram::new(
            &[InstanceSpec {
                family: 0,
                instance: 0,
                declarations: vec![spec(0x49000020), spec(0x08000000), spec(0x49000020)],
            }],
            &tables,
        )
        .unwrap();
        assert_eq!(p.external_inputs(), [0x40000020]);
        assert_eq!(p.curves.len(), 2);
        assert_eq!(p.level(0, 0, 0), Some(0));
        p.advance(&[0], &curve, &tables).unwrap();
        assert_eq!(p.curves[1].value.linear, 0); // Selector8 precedes selector9.
        assert_eq!(p.curves[0].value.linear, 0);
        p.advance(&[0], &curve, &tables).unwrap();
        assert_eq!(p.curves[1].value.linear, 32767);
        assert_eq!(p.level(0, 0, 0), p.level(0, 0, 2));
        let before = p.level(0, 0, 1);
        assert_eq!(
            p.advance(&[], &curve, &tables),
            Err(ProgramError::InvalidInputs)
        );
        assert_eq!(p.level(0, 0, 1), before);
    }
    #[test]
    fn gates_expand_external_families_and_preserve_authored_instance_bits() {
        let (_, tables) = tables();
        let mut declaration = spec(0x49000020);
        declaration.gates = vec![0x40011000, 0x40000030];
        let p = ScalarProgram::new(
            &[
                InstanceSpec {
                    family: 0,
                    instance: 0,
                    declarations: vec![declaration],
                },
                InstanceSpec {
                    family: 1,
                    instance: 0,
                    declarations: vec![],
                },
                InstanceSpec {
                    family: 1,
                    instance: 1,
                    declarations: vec![],
                },
            ],
            &tables,
        )
        .unwrap();
        assert_eq!(
            p.external_inputs(),
            [0x40000020, 0x40011000, 0x40011800, 0x40000030]
        );
    }
    #[test]
    fn rejects_missing_scalar_and_noncontiguous_instances() {
        let (_, tables) = tables();
        assert!(matches!(
            ScalarProgram::new(
                &[InstanceSpec {
                    family: 0,
                    instance: 1,
                    declarations: vec![]
                }],
                &tables
            ),
            Err(ProgramError::InvalidInstances)
        ));
        assert!(matches!(
            ScalarProgram::new(
                &[InstanceSpec {
                    family: 0,
                    instance: 0,
                    declarations: vec![spec(0x08000001)]
                }],
                &tables
            ),
            Err(ProgramError::MissingScalar(0x08000001))
        ));
    }
}

mod mxb;
pub(crate) use mxb::authored_references as authored_declaration_references;
