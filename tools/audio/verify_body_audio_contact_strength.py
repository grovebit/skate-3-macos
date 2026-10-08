"""Replay the owned base-disc Collision +C4 publisher against production Rust.

Uses the private VMX interpreter's existing dot3 lane convention, not a Xenon
hardware trace. Scalar accumulation, branches and publication run unhooked.
"""
import argparse
import hashlib
import json
from pathlib import Path
import random
import shutil
import struct
import subprocess
import sys
import tempfile


def bits(value):
    return struct.unpack(">I", struct.pack(">f", value))[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--analysis-dir", type=Path, required=True)
    parser.add_argument("--rustc", default=shutil.which("rustc"))
    args = parser.parse_args()
    if args.rustc is None:
        parser.error("rustc was not found; supply --rustc")
    image = args.analysis_dir / "default.pe"
    if hashlib.sha256(image.read_bytes()).hexdigest() != "ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42":
        parser.error("default.pe does not match the verified base-disc mapped image")
    sys.path.insert(0, str(args.analysis_dir.resolve()))
    from vmx_interp import VmxMachine

    root = Path(__file__).resolve().parents[2]
    rng = random.Random(0x82BAE368)
    fixtures, expected = [], []
    for case in range(1000):
        initial = [bits(rng.uniform(-0.1, 1.1)) for _ in range(2)]
        rows = []
        for region in range(8):
            part = 0xFFFFFFFF if rng.randrange(4) == 0 else region + 1
            force = rng.choice([bits(0.5)-1, bits(0.5), bits(0.5)+1,
                                bits(0.0), bits(rng.uniform(0, 10))])
            # Include delta-squared comparison boundaries and normal signs.
            delta = [0, rng.choice([bits(0.1**0.5)-1, bits(0.1**0.5),
                                    bits(0.1**0.5)+1, bits(-1.0),
                                    bits(rng.uniform(-2, 2))]), 0, 0]
            normal = [0, rng.choice([bits(1.0), bits(-1.0), bits(0.0)]), 0, 0]
            if case % 3 == 0:
                delta = [bits(rng.uniform(-2, 2)) for _ in range(3)] + [0]
                normal = [bits(rng.uniform(-1, 1)) for _ in range(3)] + [0]
            rows.append([part, force, *delta, *normal, bits(rng.uniform(0.001, 0.1))])
        m = VmxMachine()
        feedback, block, physical = 0x100000, 0x102050, 0x104000
        m.writable = [(feedback, feedback + 0x10000), (0x207000, 0x209000)]
        m.r[1], m.r[3] = 0x208000, feedback
        m.w32(feedback + 0xFB0, physical)
        m.w32(m.r[1] - 0xD8, block)
        m.v[63] = bytes([255]) * 16
        for reg, value in [("f7", 1000.0), ("f6", 9.0), ("f5", 0.5), ("f4", 10.0)]:
            m.set_fpr(reg, value)
        m.w32(feedback + 0xFC4, initial[0])
        m.w32(feedback + 0xFC8, initial[1])
        for region, row in enumerate(rows):
            part, force = row[:2]
            m.w32(feedback + 0x4B0 + 4 * region, part)
            m.w32(feedback + 0x3B0 + 4 * region, force)
            m.write(feedback + 0x3F0 + 16 * region, struct.pack(">4I", *row[6:10]))
            if part != 0xFFFFFFFF:
                m.write(physical + 0xFD0 + 16 * part, struct.pack(">4I", *row[2:6]))
                m.w32(physical + 0x11D0 + 4 * part, row[10])
        m.call(0x82BAE1D8, sentinel=0x82BAE39C)
        expected.append(tuple([m.u32(block + 0x74), m.u32(block + 0x78)] +
                              [m.u32(block + 4 * i) for i in range(8)]))
        fixtures.append(" ".join(map(str, initial + [v for row in rows for v in row])))

    harness = r'''
use std::io::{self, Read};
mod physics {
    #[path = ARITHMETIC]
    pub mod native_arithmetic;
    pub mod skeleton_body {
        pub const ANIMATION_PART_COUNT: usize = 24;
        pub struct SkeletonPhysicalRecord { pub velocity_changes: [[f32;4];26] }
        #[derive(Clone, Copy, Default)]
        pub struct ContactRegion {
            pub part: Option<usize>, pub force: f32, pub normal: [f32;4],
            pub material_flags: u32,
        }
        #[path = SOURCE]
        pub mod contact_audio;
    }
}
use physics::skeleton_body::*;
use physics::skeleton_body::contact_audio::*;
fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    for line in input.lines() {
        let v: Vec<u32> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
        let mut retained = [f32::from_bits(v[0]), f32::from_bits(v[1]), 0.0];
        let mut physical = SkeletonPhysicalRecord { velocity_changes: [[0.;4];26] };
        let mut weights = [0.;24];
        let mut regions = [ContactRegion::default();8];
        for i in 0..8 {
            let row = &v[2+i*11..2+(i+1)*11];
            if row[0] != u32::MAX {
                let part = row[0] as usize;
                regions[i].part = Some(part);
                physical.velocity_changes[part] = std::array::from_fn(|j| f32::from_bits(row[2+j]));
                weights[part] = f32::from_bits(row[10]);
            }
            regions[i].force = f32::from_bits(row[1]);
            regions[i].normal = std::array::from_fn(|j| f32::from_bits(row[6+j]));
        }
        let settings = BodyAudioSettings {
            strength_scale: 10., force_rate: 0.5, normal_rate: 9., divisor: 1000.,
        };
        let mut audio = BodyContactAudio::default();
        audio.update(&regions, &physical, &weights, &settings, &mut retained);
        print!("{} {}", retained[0].to_bits(), retained[1].to_bits());
        for contact in audio.current { print!(" {}", contact.intensity.to_bits()); }
        println!();
    }
}
'''.replace("SOURCE", json.dumps(str(root / "crates/skate-core/src/physics/skeleton_body/contact_audio.rs")))
    harness = harness.replace("ARITHMETIC", json.dumps(str(root / "crates/skate-core/src/physics/native_arithmetic.rs")))
    with tempfile.TemporaryDirectory(prefix="body-contact-strength-") as temp:
        rust, binary = Path(temp) / "main.rs", Path(temp) / "verify"
        rust.write_text(harness)
        subprocess.run([args.rustc, "--edition=2024", "-O", "-A", "dead_code",
                        str(rust), "-o", str(binary)], check=True)
        result = subprocess.run([str(binary)], input="\n".join(fixtures) + "\n",
                                text=True, capture_output=True, check=True)
    actual = [tuple(map(int, line.split())) for line in result.stdout.splitlines()]
    if len(actual) != len(expected):
        raise RuntimeError(f"Expected {len(expected)} results, received {len(actual)}")
    for i, (a, e) in enumerate(zip(actual, expected)):
        if a != e:
            raise RuntimeError(f"Fixture {i}: Rust {a}, native {e}: {fixtures[i]}")
    print("1000 native/Rust fixtures matched retained sums and eight current region strengths")


if __name__ == "__main__":
    main()
