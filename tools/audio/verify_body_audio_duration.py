"""Compare the duration adapter with owned base-disc instruction blocks.

Requires the local ppc_interp.py, inspect_pe.py and mapped default.pe from
audio research; no executable bytes are copied into the repository.
"""
import argparse
import json
import hashlib
from pathlib import Path
import random
import shutil
import subprocess
import struct
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--analysis-dir", type=Path, required=True)
    parser.add_argument("--rustc", default=shutil.which("rustc"))
    args = parser.parse_args()
    if args.rustc is None:
        parser.error("rustc was not found; supply --rustc")
    image = args.analysis_dir / "default.pe"
    expected_hash = "ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42"
    if hashlib.sha256(image.read_bytes()).hexdigest() != expected_hash:
        parser.error("default.pe does not match the verified base-disc mapped image")
    sys.path.insert(0, str(args.analysis_dir.resolve()))
    from ppc_interp import Machine

    source = Path(__file__).resolve().parents[2] / "crates/skate-core/src/audio/hom/duration.rs"
    rng = random.Random(0x82D82F30)
    counts = [0, 1, 59, 60, 0xFFFFFFFE, 0xFFFFFFFF, 0x7FFFFFFF,
              0x80000000, 0xFFFFFF, 0x1000000, 0x1000001]
    floats = [0, 0x80000000, 1, 0x80000001, 0x3F800000, 0xBF800000,
              0x7F800000, 0xFF800000, 0x7FC00000]
    fixtures, expected = [], []
    for i in range(1000):
        total = counts[i % len(counts)] if i < 200 else rng.getrandbits(32)
        unsettled = rng.getrandbits(32)
        motion = [0, 1, 255][i % 3]
        settled = [0, 1, 255][(i // 3) % 3]
        scoring = floats[(i // 9) % len(floats)] if i < 200 else rng.getrandbits(32)
        # Finite arbitrary strength bits verify copying, including signed zero.
        strength = rng.getrandbits(32) & 0xFEFFFFFF
        m = Machine()
        worker, table, skeleton, score, packet, collision = [
            0x100000, 0x102000, 0x103000, 0x104000, 0x105000, 0x106000]
        m.writable = [(worker, worker + 0x10000), (0x207000, 0x209000)]
        m.r[1] = 0x208000
        m.r[31] = worker
        m.r[20] = motion
        m.w32(worker + 0x7E0, total)
        m.w32(worker + 0x7E4, unsettled)
        m.w32(worker + 0x190, table)
        m.w32(table + 0x14, skeleton)
        m.write(skeleton + 0x257, bytes([settled]))
        # Stop at the blocked-counter continuation; no computation is hooked.
        m.hooks[0x82D847FC] = lambda machine: None
        m.call(0x82D8475C, sentinel=0x82D84790)
        actual_total, actual_unsettled = m.u32(worker + 0x7E0), m.u32(worker + 0x7E4)
        m.steps = 0
        m.r[30], m.r[31] = worker, score
        m.set_fpr("f0", struct.unpack(">f", m.read(0x82084998, 4))[0])
        m.call(0x82D82F30, sentinel=0x82D82F50)
        duration = m.u32(score + 0x4E8)
        m.steps = 0
        m.r[29], m.r[30], m.r[3], m.r[6] = score, packet, table, 0x82260000
        m.w32(score + 0x4E8, scoring)
        m.w32(table + 0x18, collision)
        m.w32(collision + 0xC4, strength)
        # Hook only the omitted-section exit, after the original comparison.
        m.hooks[0x82786B18] = lambda machine: None
        m.call(0x82786A18, sentinel=0x82786A3C)
        present = int(packet + 0x64 in m.mem)
        expected.append((actual_total, actual_unsettled, duration, present,
                         m.u32(packet + 0x68) if present else 0))
        fixtures.append(f"{total} {unsettled} {motion} {settled} {scoring} {strength}")

    harness = r'''
use std::io::{self, Read};
#[path = GATE]
mod gate;
#[path = SOURCE]
mod duration;
fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    for line in input.lines() {
        let v: Vec<u32> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
        let mut state = duration::WipeoutDuration {
            total_updates: v[0], unsettled_updates: v[1],
        };
        state.advance(v[2] as u8, v[3] as u8);
        let strength = duration::channel_5_strength(f32::from_bits(v[4]), f32::from_bits(v[5]));
        println!("{} {} {} {} {}", state.total_updates, state.unsettled_updates,
            state.published_duration().to_bits(), u8::from(strength.is_some()),
            strength.map(f32::to_bits).unwrap_or(0));
    }
}
'''.replace("SOURCE", json.dumps(str(source))).replace("GATE", json.dumps(str(source.with_name('gate.rs'))))
    with tempfile.TemporaryDirectory(prefix="body-audio-duration-") as temp:
        rust, binary = Path(temp) / "main.rs", Path(temp) / "verify"
        rust.write_text(harness)
        subprocess.run([args.rustc, "--edition=2024", "-O", str(rust), "-o", str(binary)], check=True)
        result = subprocess.run([str(binary)], input="\n".join(fixtures) + "\n",
                                text=True, capture_output=True, check=True)
    actual = [tuple(map(int, line.split())) for line in result.stdout.splitlines()]
    if len(actual) != len(expected):
        raise RuntimeError(f"Expected {len(expected)} results, received {len(actual)}")
    for i, (a, e) in enumerate(zip(actual, expected)):
        if a != e:
            raise RuntimeError(f"Fixture {i}: {fixtures[i]}: Rust {a}, native {e}")
    print("1000 native/Rust fixtures matched counters, duration bits and section presence/strength")


if __name__ == "__main__":
    main()
