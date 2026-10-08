"""Replay the owned manager clock against production Rust and timing research.

The provider, upstream clamp and stamp checks are native-only research checks,
not a port of the publication adapter. Replays the owned instructions from
your own default.xex; embeds no executable bytes.
"""
import argparse
import json
from pathlib import Path
import random
import shutil
import struct
import subprocess
import tempfile

from tools.owned_game import xex
from tools.native_replay.ppc_interp import Machine


def check(condition, message):
    if not condition:
        raise RuntimeError(message)


def bits(value):
    return struct.unpack(">I", struct.pack(">f", value))[0]


def float32(value):
    return struct.unpack(">f", struct.pack(">f", value))[0]


def verify_clock(image, rustc):
    source = Path(__file__).resolve().parents[2] / "crates/skate-core/src/audio/clock.rs"
    rng = random.Random(0x82473060)
    rows = [(float32(1 / 60), True, False)] * 60
    rows += [(d, e, f) for d in [0.0, 0.004, 0.006, float32(0.02),
                               float32(0.020000001), 0.03, 1.9]
             for e, f in [(False, True), (True, False), (True, True)]]
    rows += [(float32(rng.choice([0.0, -0.0, 1 / 60, 0.005, 0.02,
                                 0.020000001, 0.03, rng.uniform(0, 0.05)])),
              rng.randrange(6) != 0, rng.randrange(7) == 0) for _ in range(2000)]
    m = Machine(image)
    manager = 0x100000
    m.writable = [(manager, manager + 0x1000)]
    # Disabled exit is outside the scheduling block, before any consumers.
    m.hooks[0x824732D8] = lambda machine: None
    expected, fixtures = [], []
    for delta, enabled, force in rows:
        delta = float32(delta)
        m.r[3], m.r[5] = manager, int(force)
        m.write(manager + 0x28, bytes([enabled]))
        m.set_fpr("f1", delta)
        m.steps = 0
        m.call(0x82473074, sentinel=0x82473124)
        selected = [enabled and m.r[30] != 0, enabled and m.r[28] != 0]
        expected.append(" ".join(str(bits(m.fpr(f"f{reg}"))) if yes else "-"
                                 for yes, reg in zip(selected, [31, 30])))
        fixtures.append(f"{bits(delta)} {int(enabled)} {int(force)}")
    harness = r'''
use std::io::{self, Read};
#[path = SOURCE]
mod clock;
fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut clock = clock::AudioClock::default();
    for line in input.lines() {
        let v: Vec<u32> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
        let phases = clock.advance(f32::from_bits(v[0]), v[1] != 0, v[2] != 0);
        let field = |v: Option<f32>| v.map_or("-".into(), |f| f.to_bits().to_string());
        println!("{} {}", field(phases.inputs), field(phases.mixmap));
    }
}
'''.replace("SOURCE", json.dumps(str(source)))
    with tempfile.TemporaryDirectory(prefix="audio-manager-cadence-") as temp:
        rust, binary = Path(temp) / "main.rs", Path(temp) / "verify"
        rust.write_text(harness)
        subprocess.run([rustc, "--edition=2024", "-O", str(rust), "-o", str(binary)], check=True)
        result = subprocess.run([str(binary)], input="\n".join(fixtures) + "\n",
                                text=True, capture_output=True, check=True)
    actual = result.stdout.splitlines()
    check(len(actual) == len(expected), "Clock result count differs")
    for i, (a, e) in enumerate(zip(actual, expected)):
        check(a == e, f"Clock fixture {i}: {fixtures[i]}: Rust {a}, native {e}")
    print(f"{len(rows)} native/Rust clock cases passed (60 Hz, variable, disabled, forced, threshold)")


def verify_research(image):
    # Start after the atomic +44 read. The external readiness and pause queries
    # are controlled inputs; all three guard branches execute owned instructions.
    for state_value in [0, 17, 18, 19]:
        for ready in [False, True]:
            for paused in [False, True]:
                m = Machine(image)
                root, publication, table = 0x100000, 0x101000, 0x102000
                m.w32(0x83027D34, root)
                m.w32(root + 0x98, publication)
                m.w32(publication, table)
                m.w32(table + 0x5C, 0x200000)
                m.w32(0x83073544, 0x103000)
                m.r[29], m.r[10] = 0x83020000, state_value
                queries = []

                def readiness(machine):
                    queries.append("ready")
                    machine.r[3] = int(ready)

                def pause_query(machine):
                    check(machine.r[4] == 0x10, "Publication pause query mask")
                    queries.append("pause")
                    machine.r[3] = int(paused)

                def skipped(machine):
                    queries.append("skip")
                    machine.lr = 0x826B9A5C

                m.hooks[0x200000] = readiness
                m.hooks[0x824C2A50] = pause_query
                m.hooks[0x826B9AFC] = skipped
                m.call(0x826B99FC, sentinel=0x826B9A5C)
                expected = ["ready"] if state_value == 18 else []
                if state_value == 18 and ready:
                    expected.append("pause")
                if state_value != 18 or not ready or paused:
                    expected.append("skip")
                check(queries == expected, f"Publication guards: {queries} != {expected}")
    print("16 native-only publication guard cases passed (including skip/query ordering)")

    # These checks do not substitute synthetic indices for a runtime producer.
    # Stop before phase/index selection; hook only the external index and timing
    # queries. In particular, no native ratio computation is hooked.
    m = Machine(image)
    state, provider, context, table, output = [0x100000 + i * 0x1000 for i in range(5)]
    m.writable = [(state, output + 0x1000)]
    m.w32(provider + 4, state)
    m.w32(state + 8, context)
    m.w32(context, table)
    m.w32(table, 0x200000)
    m.wf32(state + 0x20, float32(1 / 60))
    m.wf32(state + 0x24, float32(1 / 60))
    m.wf32(state + 0x10, 0.375)
    m.w32(state + 0xC, 41)
    m.hooks[0x82D92F00] = lambda machine: setattr(machine, "lr", 0x82D92DEC)
    # delta, queried index, timing query value, resulting retained denominator.
    rows = [(1 / 60, 42, 1 / 30, 1 / 30),
            (0, 43, 1 / 120, 1 / 30),
            (-1, 43, 1 / 120, 1 / 30),
            (1 / 60, 0x01FFFFFF, 1 / 120, 1 / 30),
            (1 / 60, 43, 1 / 120, 1 / 120),
            (0, 44, 1 / 60, 1 / 120),
            (1 / 60, 44, 0, 0),
            (0, 45, 1 / 60, 0),
            (1 / 60, 45, -0.5, -0.5),
            (1 / 60, 46, 1 / 60, 1 / 60)]
    for i, (delta, index, queried, retained) in enumerate(rows):
        queries = []

        def index_query(machine):
            queries.append("index")
            machine.r[3] = index

        def timing_query(machine):
            queries.append("timing")
            machine.set_fpr("f1", float32(queried))

        m.hooks[0x82D919F8] = index_query
        m.hooks[0x200000] = timing_query
        m.r[3] = state
        m.set_fpr("f1", float32(delta))
        m.steps = 0
        m.call(0x82D92D94, sentinel=0x82D92DEC)
        expected_queries = [] if delta <= 0 else ["index"]
        if delta > 0 and index != 0x01FFFFFF:
            expected_queries.append("timing")
        check(queries == expected_queries, f"Provider query order {i}: {queries}")
        check(m.u32(state + 0x24) == bits(retained), f"Provider denominator {i}")
        m.r[3], m.r[4] = output, provider
        m.steps = 0
        m.call(0x82D93068)
        ratio = float32(float32(1 / 60) / float32(retained)) if retained > 0 else 0.0
        check(m.u32(output + 8) == bits(ratio), f"Provider ratio {i}")
        check(m.u32(output) == 41 and m.u32(output + 4) == bits(0.375)
              and m.u32(output + 12) == 1, f"Provider descriptor {i}")
        # A skipped publication does not invoke advance/read dispatch: re-querying
        # the descriptor must leave both its value and the provider state intact.
        saved_state, saved_output = m.read(state, 0x28), m.read(output, 16)
        m.r[3], m.r[4] = output, provider
        m.call(0x82D93068)
        check(m.read(state, 0x28) == saved_state and m.read(output, 16) == saved_output,
              f"Retained descriptor {i}")
    print(f"{len(rows)} native-only provider retention/query-order cases passed; phase/index not replayed")

    packet, target = 0x110000, 0x111000
    m.writable += [(packet, target + 0x4000)]
    for offset in [-1, 0, 64]:
        for fraction in [-0.5, 0.0, 0.25, 1.0, 1.5]:
            m.r[23], m.r[28] = packet, target
            m.write(packet + 0x18, struct.pack(">h", offset))
            m.wf32(packet + 64, 0.75)
            m.wf32(target + 0x30A4, 0.125)
            m.write(target + 0x30A0, b"\xff")
            m.set_fpr("f1", fraction)
            m.steps = 0
            m.call(0x82788A74, sentinel=0x82788AB8)
            check(m.read8(target + 0x30A0) == int(offset > 0), "Section A presence")
            check(m.u32(target + 0x30A4) == bits(0.75 if offset > 0 else 0.125),
                  "Section A copy/retention; fraction must not affect strength")
    print("15 native-only section-A copy/retention cases passed across five fractions")

    values = [0.0, 1 / 60, 0.02, 0.03, 1.0, 1.8999998, 1.9, 2.0, 10.0]
    for value in values:
        m.r[23] = 0x820A0000
        m.set_fpr("f31", float32(value))
        m.steps = 0
        m.call(0x8293EA90, sentinel=0x8293EAA0)
        check(bits(m.fpr("f31")) == bits(min(float32(value), float32(1.9))), "Loop clamp")
    print(f"{len(values)} native-only upstream 1.9-second clamp cases passed")
    for stamp in [0, 1, 0xFFFFFFFE, 0xFFFFFFFF]:
        for mask in [0, 1, 2, 3]:
            m.r[3], m.r[4] = state, mask
            m.w32(state + 0x10, stamp)
            m.w32(state + 0x14, 17)
            m.steps = 0
            m.call(0x82834E0C, sentinel=0x82834E38)
            check(m.u32(state + 0x10) == (stamp + (mask & 1)) & 0xFFFFFFFF, "Group stamp")
            check(m.u32(state + 0x14) == 17 + ((mask >> 1) & 1), "Secondary stamp")
    print("16 native-only update-mask stamp cases passed (including wrap)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", type=Path, required=True,
                        help="your Skate 3 folder, with default.xex")
    parser.add_argument("--rustc", default=shutil.which("rustc"))
    args = parser.parse_args()
    if args.rustc is None:
        parser.error("rustc was not found; supply --rustc")
    image = xex.load(args.game)

    verify_clock(image, args.rustc)
    verify_research(image)


if __name__ == "__main__":
    main()
