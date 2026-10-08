use super::*;

/// Group sections by header slot: +04 declarations, +08 modulation, +0C
/// sums, +10 outputs, +14 write maps (no header) and +18 envelopes.
#[derive(Default)]
struct Group {
    declarations: Vec<Vec<u32>>,
    modulations: Vec<Vec<u32>>,
    sums: Vec<Vec<u32>>,
    outputs: Vec<Vec<u32>>,
    maps: Vec<u32>,
    envelopes: Vec<Vec<u32>>,
}

fn program(groups: &[Option<Group>]) -> Vec<u8> {
    let mut w = vec![0, groups.len() as u32, 16, 0];
    w.resize(4 + groups.len(), u32::MAX);
    for (family, group) in groups.iter().enumerate() {
        let Some(group) = group else { continue };
        let start = w.len();
        w[4 + family] = 4 * start as u32;
        w.resize(start + 8, u32::MAX);
        let mut section = |w: &mut Vec<u32>, slot: usize, records: &[Vec<u32>], buffers| {
            w[start + slot] = 4 * (w.len() - start) as u32;
            w.extend([records.len() as u32, buffers, 0, 0]);
            for record in records {
                w.extend(record);
            }
        };
        section(&mut w, 1, &group.declarations, 0);
        section(&mut w, 2, &group.modulations, 0);
        section(&mut w, 3, &group.sums, 0);
        section(&mut w, 4, &group.outputs, 1);
        w[start + 5] = 4 * (w.len() - start) as u32;
        w.extend(&group.maps);
        section(&mut w, 6, &group.envelopes, 0);
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

/// Main: declaration 0 reads declaration 1's curve; declaration 2 and the
/// bit-9 envelope 1 are outside the cone. Collision: sum 0 adds declaration
/// 0 and the Pause-triggered duck; output 0 writes slot 18 with node 0;
/// output 1 writes pitch slot 1 through the shared modulation node.
fn fixture() -> Vec<u8> {
    let main = Group {
        declarations: vec![
            vec![0x0800_0001, 0x0000_d8f0],
            vec![0x4800_0022, 0],
            vec![0x4800_0025, 0],
        ],
        envelopes: vec![
            vec![
                0xa100_0100,
                0x0000_d8f0,
                0x4000_0070,
                0x9001,
                0x9001,
                0x9001,
            ],
            vec![0xa100_0301, 0, 0x4000_0071, 1, 1, 1],
        ],
        ..Group::default()
    };
    let collision = Group {
        modulations: vec![vec![
            0x8103_0000,
            0x9003_0000,
            0x6666_0154,
            0x0032_0001,
            0x0032_0001,
            0x0032_0001,
            0x0032_0001,
        ]],
        sums: vec![vec![0x0002_0000, 0x0000_d8f0, 0x0000_0000, 0xb100_3000]],
        outputs: vec![
            vec![
                0x0002_0000,
                0x0000_d8f0,
                0x4003_0000,
                0x9003_0000,
                0x3003_0000,
            ],
            vec![0x0001_0000, 0x0000_d8f0, 0x4003_0000, 0x9003_0000],
        ],
        maps: vec![0xe000_0001, 0x4812_fbb4, 0xe100_0001, 0x0401_0000],
        ..Group::default()
    };
    program(&[Some(main), None, None, Some(collision)])
}

fn inputs(mix: &CollisionMix) -> WordInputs {
    let mut inputs = WordInputs::default();
    for owner in mix.required_inputs() {
        inputs.attach(owner).unwrap();
    }
    inputs
}

#[test]
fn cone_follows_only_selected_volume_and_pitch_outputs() {
    let (_, tables) = tables();
    let mix = CollisionMix::from_mxb(&fixture(), &[1, 0, 0, 2], &tables).unwrap();
    let cone = discover(&fixture(), &[1, 0, 0, 2], &[0x9003_0000, 0x3003_0000]).unwrap();
    assert_eq!(
        cone,
        Cone {
            declarations: [(0, 0), (0, 1)].into(),
            envelopes: [(0, 0)].into(),
            sums: [(3, 0)].into(),
            modulations: [(3, 0)].into(),
        }
    );
    assert_eq!(
        mix.required_inputs().collect::<Vec<_>>(),
        [0x4000_0020, 0x4000_0070, 0x6003_0000, 0x6003_0800]
    );
    assert_eq!(mix.groups(), 2);
    // Constructed enabled with zeroed controls (82927AD0..82927B4C).
    assert_eq!(mix.words(1).unwrap()[15], 1);
    assert_eq!(&mix.words(1).unwrap()[..15], &[0; 15]);
}

#[test]
fn phases_duck_through_the_envelope_and_disabled_outputs_write_one_entry() {
    let (curves, tables) = tables();
    let mut mix = CollisionMix::from_mxb(&fixture(), &[1, 0, 0, 2], &tables).unwrap();
    let mut words = inputs(&mix);
    words.set(0x4000_0022, 32767);
    for group in [0x6003_0000, 0x6003_0800] {
        words.set(group | 1, 5.0_f32.to_bits());
        words.set(group | 15, 1);
    }
    let slot18 = |mix: &CollisionMix, group| mix.words(group).unwrap()[9] & 0xffff;
    // Declaration 0 reads declaration 1's curve from the previous phase.
    for _ in 0..2 {
        mix.advance(1. / 30., &mut words, &curves, &tables).unwrap();
    }
    let open = slot18(&mix, 0);
    assert!(open > 0 && open < 32730, "{open}");
    assert_eq!(slot18(&mix, 1), open);
    // Pause[0] attacks the -10000 envelope; the sum clamps to -10000.
    words.set(0x4000_0070, 32767);
    for _ in 0..3 {
        mix.advance(1. / 30., &mut words, &curves, &tables).unwrap();
    }
    assert_eq!(slot18(&mix, 0), 0);
    words.set(0x4000_0070, 0);
    for _ in 0..3 {
        mix.advance(1. / 30., &mut words, &curves, &tables).unwrap();
    }
    assert_eq!(slot18(&mix, 0), open);
    // A reset group writes raw D8F0 to its first map entry only (82928A68).
    mix.reset(1);
    mix.advance(1. / 30., &mut words, &curves, &tables).unwrap();
    assert_eq!(slot18(&mix, 1), 0xd8f0);
    assert_eq!(mix.words(1).unwrap()[0] & 0xffff, 0);
    mix.activate(1);
    mix.advance(1. / 30., &mut words, &curves, &tables).unwrap();
    assert_eq!(slot18(&mix, 1), open);
    // Producers must attach every required buffer.
    assert_eq!(
        mix.advance(1. / 30., &mut WordInputs::default(), &curves, &tables),
        Err(MixError::MissingInput(0x4000_0020))
    );
}

#[test]
fn unsupported_reference_kinds_and_missing_selected_outputs_are_rejected() {
    let (_, tables) = tables();
    let mut bytes = fixture();
    // Point the sum at static word kind 6 (830282F4), untraced.
    let find = |bytes: &[u8], word: u32| {
        bytes
            .chunks(4)
            .position(|c| c == word.to_be_bytes())
            .unwrap()
            * 4
    };
    let at = find(&bytes, 0xb100_3000);
    bytes[at..at + 4].copy_from_slice(&0xc000_0001_u32.to_be_bytes());
    assert_eq!(
        CollisionMix::from_mxb(&bytes, &[1, 0, 0, 2], &tables).err(),
        Some(MixError::Unsupported(0xc000_0001))
    );
    let mut bytes = fixture();
    let at = find(&bytes, 0x4812_fbb4);
    bytes[at..at + 4].copy_from_slice(&0x0812_fbb4_u32.to_be_bytes());
    let at = find(&bytes, 0x0401_0000);
    bytes[at..at + 4].copy_from_slice(&0x0801_0000_u32.to_be_bytes());
    assert_eq!(
        CollisionMix::from_mxb(&bytes, &[1, 0, 0, 2], &tables).err(),
        Some(MixError::Program)
    );
}

fn owned() -> Option<(Vec<u8>, CurveTable, ScalarTables)> {
    let root = std::path::PathBuf::from(std::env::var_os("SKATE_OWNED_MIXMAP_DIR")?);
    let read = |name: &str| std::fs::read(root.join(name)).unwrap();
    Some((
        read("MixMapSK8.mxb"),
        CurveTable::from_be_bytes(&read("mixmap-curve-table.bin")).unwrap(),
        ScalarTables::from_be_bytes(
            &read("mixmap-log-table.bin"),
            &read("mixmap-volume-table.bin"),
        )
        .unwrap(),
    ))
}

/// Body slot 18 with free-skate inputs (profile SFX/dialogue volume 1.0,
/// every other cone producer idle) at camera distance `d`, phase 0. The
/// expected levels come from tools.audio.check_mixmap_modulation and
/// tools.audio.check_mixmap_volume with record level -1 and adjustment -1100.
#[test]
#[ignore = "requires the owned MixMapSK8.mxb and executable tables"]
fn owned_cone_matches_reference_levels_by_distance() {
    let (data, curves, tables) = owned().expect("SKATE_OWNED_MIXMAP_DIR");
    let mut counts = vec![0; 14];
    counts[0] = 1;
    counts[1] = 2;
    counts[3] = 10;
    let mut mix = CollisionMix::from_mxb(&data, &counts, &tables).unwrap();
    // 16 Main and 2 Player envelope records; Player has two instances.
    assert_eq!(
        (mix.envelopes.len(), mix.sums.len(), mix.modulations.len()),
        (20, 10, 10)
    );
    let mut inputs = inputs(&mix);
    inputs.set(0x4000_0022, 32767);
    inputs.set(0x4000_0023, 32767);
    let cases = [
        (1.0_f32, 9202),
        (2., 8668),
        (3., 8126),
        (5., 7053),
        (8., 5564),
        (12., 3984),
        (20., 1687),
        (35., 117),
        (60., 0),
        (0.5, 9202),
    ];
    for (group, (distance, _)) in cases.iter().enumerate() {
        let owner = 0x6003_0000 | ((group as u32) << 11);
        inputs.set(owner | 1, distance.to_bits());
        inputs.set(owner | 15, 1);
    }
    for _ in 0..4 {
        mix.advance(1. / 30., &mut inputs, &curves, &tables)
            .unwrap();
    }
    for (group, (distance, expected)) in cases.iter().enumerate() {
        let words = mix.words(group).unwrap();
        assert_eq!(words[9] & 0xffff, *expected, "{distance} m");
        // Slots 12 and 18 share the -1100 adjustment.
        assert_eq!(words[6] & 0xffff, *expected, "{distance} m");
    }
}

#[test]
#[ignore = "requires the owned MixMapSK8.mxb and executable tables"]
fn owned_pitch_cone_preserves_startup_history_hom_gates_and_reset() {
    use crate::audio::{hom, master::PublicationSpeed, pitch};
    let (data, curves, tables) = owned().expect("SKATE_OWNED_MIXMAP_DIR");
    let root = std::path::PathBuf::from(std::env::var_os("SKATE_OWNED_MIXMAP_DIR").unwrap());
    let pitch = pitch::PitchTables::from_be_bytes(
        &std::fs::read(root.join("pitch_semitones.bin")).unwrap(),
        &std::fs::read(root.join("pitch_cents.bin")).unwrap(),
    )
    .unwrap();
    let mut counts = vec![0; 14];
    counts[0] = 1;
    counts[1] = 2;
    counts[3] = 10;
    let mut mix = CollisionMix::from_mxb(&data, &counts, &tables).unwrap();
    assert_eq!(mix.outputs.len(), 40);
    assert_eq!(mix.modulations.len(), 10); // Record 2's node 1 is excluded.
    assert!(mix.scalars.level(3, 0, 0).is_none());
    let mut inputs = inputs(&mix);
    PublicationSpeed::evaluate(Some(1.0), false, 1.0)
        .unwrap()
        .write(inputs.words_mut(0x40000020).unwrap());
    for i in 0..10 {
        let owner = 0x60030000 | (i << 11);
        inputs.set(owner | 1, 5.0_f32.to_bits());
        inputs.set(owner | 15, 1);
    }
    let cents = |mix: &CollisionMix| {
        (
            (mix.words(0).unwrap()[0] >> 16) as i16,
            mix.words(0).unwrap()[11] as i16,
        )
    };
    mix.advance(1.0 / 30.0, &mut inputs, &curves, &tables)
        .unwrap();
    assert_eq!(cents(&mix), (0, 0)); // Main6 reads Main8's constructed curve.
    mix.advance(1.0 / 30.0, &mut inputs, &curves, &tables)
        .unwrap();
    assert_eq!(cents(&mix), (-4, 0));
    for (material, class, authored, combined) in [
        (0x62, 5, 3796, 3786),
        (0x5f, 6, 4096, 4086),
        (0x60, 6, 3096, 3088),
        (0x61, 9, 4096, 4096),
    ] {
        let slot = pitch::pitch_slot(material, class);
        let voice =
            pitch::collision_pitch(authored, mix.words(0).map(|w| w.as_slice()), slot, &pitch)
                .unwrap();
        assert_eq!(voice.combined_pitch, combined);
    }
    // HOM and treatment are reachable evaluator inputs, but the host does
    // not synthesize their still-provisional manager flags from bail state.
    let mut hom = hom::State::default();
    let mut results = Vec::new();
    for (active, alternate, treatment) in [(0, 0, 1.0), (1, 0, 1.0), (1, 1, 1.0), (1, 0, 0.5)] {
        PublicationSpeed::evaluate(Some(0.5), active != 0, treatment)
            .unwrap()
            .write(inputs.words_mut(0x40000020).unwrap());
        hom.publish(
            &hom::Inputs {
                manager_34b: active,
                manager_34c: alternate,
                timing_ratio: 0.5,
                secondary_flags: None,
                manager_49c: 0,
                manager_36c: 0,
                manager_1cc: 0,
            },
            true,
            |slot, value| inputs.words_mut(0x400000c0).unwrap()[slot] = value,
        );
        for _ in 0..2 {
            mix.advance(1.0 / 30.0, &mut inputs, &curves, &tables)
                .unwrap();
        }
        results.push(cents(&mix));
    }
    assert_ne!(results[0], results[1]);
    assert_eq!(results[1], results[2]); // HOM latch preserves the first branch.
    assert_eq!(results[1], results[3]); // HOM gate bypasses Main6 speed depth.
    mix.reset(0);
    mix.advance(1.0 / 30.0, &mut inputs, &curves, &tables)
        .unwrap();
    assert_eq!(cents(&mix), (0, 0));
    assert_eq!(
        pitch::collision_pitch(3796, None, 1, &pitch).unwrap().ratio,
        0.0
    );
}
