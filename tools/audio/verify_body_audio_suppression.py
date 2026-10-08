"""Replay the wipeout Collision+D6 suppression writer against production Rust.

Owned base-disc instructions are loaded only from --analysis-dir. Other
material branches are exit hooks: their behavior is outside this port.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--analysis-dir", type=Path, required=True)
    parser.add_argument("--rustc", default=shutil.which("rustc"))
    args = parser.parse_args()
    if args.rustc is None:
        parser.error("rustc was not found; supply --rustc")
    expected_hash = "ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42"
    if hashlib.sha256((args.analysis_dir / "default.pe").read_bytes()).hexdigest() != expected_hash:
        parser.error("default.pe does not match the verified base-disc mapped image")
    sys.path.insert(0, str(args.analysis_dir.resolve()))
    from ppc_interp import Machine

    root = Path(__file__).resolve().parents[2]
    source = root / "crates/skate-game/src/physics/player_state/suppression.rs"
    gate = root / "crates/skate-core/src/audio/hom/gate.rs"
    states = [0, 99, 100, 104, 105, 200, 201, 299, 300, 301,
              500, 501, 502, 503, 600, 601, 602, 603, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF]
    fixtures, expected = [], []
    for state in states:
        for contact in [0, 1, 2, 127, 128, 255]:
            for prior in [0, 1, 2, 127, 128, 255]:
                for state44 in [0, 1, 255]:
                    m = Machine()
                    owner, table, processed, collision, packet, feedback, worker, filtered = [
                        0x100000 + 0x2000 * i for i in range(8)]
                    m.writable = [(owner, owner + 0x10000), (0x207000, 0x209000)]
                    m.r[1] = 0x208000
                    m.w32(owner + 0x708, table)
                    m.w32(owner + 0x70C, processed)
                    m.w32(table + 0x18, collision)
                    m.w32(table + 0x1C, packet)
                    m.w32(processed + 0x9CC, state)
                    m.write(feedback + 0xFE4, bytes([contact]))
                    # The original load and copy from skeleton contact+FE4 to Collision+D6.
                    m.r[3] = feedback
                    m.call(0x82BADA0C, sentinel=0x82BADA10)
                    m.r[8] = collision + 0x50
                    m.call(0x82BADA78, sentinel=0x82BADA7C)
                    if m.read8(collision + 0xD6) != contact:
                        raise RuntimeError("Skeleton material-6 publication did not copy its byte")
                    m.write(packet + 0x45, bytes([prior]))
                    m.r[3] = owner
                    # Exit before unported state branches and after the ported write.
                    for address in [0x82D8D610, 0x82D8D5B8, 0x82D8D550,
                                    0x82D8D62C, 0x82D8D46C, 0x82D8D824]:
                        m.hooks[address] = lambda machine: None
                    m.call(0x82D8D430, sentinel=0x82D8D4F8)
                    result = m.read8(packet + 0x45)
                    # Worker primary gate consumes the completed same packet.
                    m.w32(worker + 0x190, table)
                    m.w32(table + 0x40, filtered)
                    m.w32(filtered, 4)
                    m.write(packet + 0x44, bytes([state44]))
                    m.r[3] = worker
                    m.call(0x82D8362C, sentinel=0x82D83690)
                    expected.append((result, m.r[11]))
                    fixtures.append(f"{state} {contact} {prior} {state44}")

    harness = r'''
use std::io::{self, Read};
#[path = SOURCE]
mod suppression;
#[path = GATE]
mod gate;
fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    for line in input.lines() {
        let v: Vec<u32> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
        let result = suppression::wipeout_contact(v[0], v[1] as u8, v[2] as u8);
        let mut worker = gate::GateState::default();
        worker.advance(4, v[3] as u8, result);
        println!("{} {}", result, worker.active_8b4);
    }
}
'''.replace("SOURCE", json.dumps(str(source))).replace("GATE", json.dumps(str(gate)))
    with tempfile.TemporaryDirectory(prefix="body-audio-suppression-") as temp:
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
    print(f"{len(expected)} native/Rust fixtures matched dispatch, retained bytes and worker gate")


if __name__ == "__main__":
    main()
