"""Replay Gai0's initial/changed 256-frame render against the existing Rust ramp.

Requires the owned mapped image and local VMX interpreter. No proprietary
bytes are embedded. This checks the node, not the game's thread scheduling.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools.audio.verify_aems_selection import IMAGE_SHA256, replay_class


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--analysis-dir', type=Path, required=True)
    parser.add_argument('--rustc', default=shutil.which('rustc'))
    args = parser.parse_args()
    if not args.rustc:
        parser.error('rustc is required')
    if hashlib.sha256((args.analysis_dir / 'default.pe').read_bytes()).hexdigest() != IMAGE_SHA256:
        parser.error('default.pe does not match the verified base-disc mapped image')
    sys.path.insert(0, str(args.analysis_dir.resolve()))
    from vmx_interp import VmxMachine

    class Render(replay_class(VmxMachine)):
        def step(self, pc):
            mn, ops, _ = self.decode(pc)
            if mn == 'vmr':
                self.setv(ops[0], self.vr(ops[1]))
                return pc + 4
            if mn in ('dcbt', 'dcbtst'):
                return pc + 4  # Cache prefetch only.
            if mn in ('dcbz', 'dcbz128', 'dcbzl'):
                address = ((0 if ops[0] in ('r0', '0') else self.gpr(ops[0]))
                           + self.gpr(ops[1])) & 0xffffffff
                self.write(address & ~127, bytes(128))
                return pc + 4
            return super().step(pc)

    fixtures, expected = [], []
    values = [0., .125, .5, 1., 2., struct.unpack('>f', bytes.fromhex('3f6e331d'))[0],
              struct.unpack('>f', bytes.fromhex('3e4b4f96'))[0]]
    for initial in (0, 1):
        for prior in values:
            for target in values:
                m = Render()
                m.writable = [(0x100000, 0x110000), (0x200000, 0x210000)]
                m.r[1] = 0x208000
                node, context, input_desc, output_desc = 0x100000, 0x101000, 0x102000, 0x103000
                source, output = 0x104000, 0x105000
                m.wf32(node + 0x34, target)
                m.wf32(node + 0x38, prior)
                m.write(node + 0x2a, bytes([1]))
                m.w32(context + 0x1c, input_desc)
                m.w32(context + 0x20, output_desc)
                for descriptor, pcm in ((input_desc, source), (output_desc, output)):
                    m.w32(descriptor + 4, pcm)
                    m.write(descriptor + 0xe, struct.pack('>H', 256))
                m.write(source, struct.pack('>256f', *([1.] * 256)))
                m.r[3], m.r[4], m.r[5] = node, context, initial
                m.call(0x82afb4e0)
                if m.u32(node + 0x38) != struct.unpack('>I', struct.pack('>f', target))[0]:
                    raise RuntimeError('Gai0 did not retain the target for the following block')
                if m.u32(context + 0x1c) != output_desc:
                    raise RuntimeError('Gai0 output buffer swap mismatch')
                expected.append(struct.unpack('>256I', m.read(output, 1024)))
                p, t = [struct.unpack('>I', struct.pack('>f', v))[0] for v in (prior, target)]
                fixtures.append(f'{initial} {p} {t}')
    source = Path(__file__).resolve().parents[2] / 'crates/skate-core/src/audio/pan/block.rs'
    harness = r'''
#[path = SOURCE]
mod block;
use std::io::{self, Read};
fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    for line in input.lines() {
        let v: Vec<u32> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
        let target = f32::from_bits(v[2]);
        let prior = if v[0] != 0 { target } else { f32::from_bits(v[1]) };
        let gains = block::channel_gains(prior, target);
        println!("{}", gains.iter().map(|x| x.to_bits().to_string()).collect::<Vec<_>>().join(" "));
    }
}
'''.replace('SOURCE', json.dumps(str(source)))
    with tempfile.TemporaryDirectory(prefix='aems-start-') as temp:
        rust, binary = Path(temp) / 'main.rs', Path(temp) / 'verify'
        rust.write_text(harness)
        subprocess.run([args.rustc, '--edition=2024', '-O', str(rust), '-o', str(binary)], check=True)
        result = subprocess.run([str(binary)], input='\n'.join(fixtures) + '\n',
                                text=True, capture_output=True, check=True)
    actual = [tuple(map(int, line.split())) for line in result.stdout.splitlines()]
    if len(actual) != len(expected):
        raise RuntimeError('Rust fixture count mismatch')
    for i, (a, e) in enumerate(zip(actual, expected)):
        if a != e:
            raise RuntimeError(f'Gai0 fixture {i} differs: {fixtures[i]}')
    print(f'{len(fixtures)} Gai0 native/Rust blocks matched all 256 gain words, initial flag and retained target')


if __name__ == '__main__':
    main()
