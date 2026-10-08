"""Replay collision pitch against owned base-disc instructions and production Rust.

No executable bytes are embedded. The independent native layout uses the owned
modulation constructor and scalar/output field layouts established by the binders.
"""
import argparse
import hashlib
import os
from pathlib import Path
import random
import shutil
import struct
import subprocess
import tempfile

from tools.native_replay import xex_image
from tools.native_replay.ppc_interp import Machine, f32_bits
from tools.owned_game import collision_mix


def check(actual, expected, context):
    if actual != expected:
        raise RuntimeError(f'{context}: Rust {actual}, native {expected}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', type=Path, required=True,
                        help='your Skate 3 folder, with default.xex')
    args = parser.parse_args()
    executable = xex_image.load(args.game)
    data = (args.game / 'data/audio' / collision_mix.PROGRAM[0]).read_bytes()
    check(hashlib.sha256(data).hexdigest(), collision_mix.PROGRAM[1], 'program hash')
    tables = collision_mix.executable_tables((args.game / 'default.xex').read_bytes(),
                                             lambda xex: executable)
    root = Path(__file__).resolve().parents[2]
    word = lambda offset: struct.unpack_from('>I', data, offset)[0]
    m = Machine(executable)
    cursor = 0x100000

    def alloc(size):
        nonlocal cursor
        result = cursor
        cursor += (size + 15) & ~15
        m.write(result, bytes(size))
        return result

    image = alloc(len(data))
    m.write(image, data)
    evaluator = alloc(4096)
    m.writable = [(0x100000, 0x300000), (0x400000, 0x410000)]

    def call(address, stop=None):
        m.steps = 0
        m.r[1] = 0x408000
        if stop is None:
            m.call(address)
        else:
            m.call(address, sentinel=stop)

    # Native modulation construction. Select only node 0 for pitch/volume;
    # node 1 belongs to excluded output record 2.
    for offset in [0x1a0, 0x1a4, 0x1a8]:
        m.w32(evaluator + offset, alloc(65536))
    group = word(word(8) + 12)
    objects = alloc(10 * 96)
    nodes = []
    for instance in range(10):
        obj = objects + instance * 96
        for offset, value in [(4, evaluator), (0x1c, image + group), (0x40, instance), (0x5c, objects)]:
            m.w32(obj + offset, value)
        m.r[3] = obj
        call(0x8292b680)
        pair = m.u32(obj + 0x14)
        meta, run = m.u32(pair), m.u32(pair + 4)
        inputs = alloc(64)
        m.w32(run + 4, inputs)
        nodes.append((pair, meta, run, inputs))
    pairs = alloc(80)
    for i, (_, meta, run, _) in enumerate(nodes):
        m.w32(pairs + i * 8, meta)
        m.w32(pairs + i * 8 + 4, run)
    m.w32(evaluator + 0x1a0, pairs)
    m.w32(evaluator + 0x1d4, 10)

    # Parse and follow the six added declarations, preserving authored order.
    main_group = word(word(8))
    section = main_group + word(main_group + 4)
    at = section + 16
    declarations = {}
    for index in range(word(section)):
        source, descriptor = word(at), word(at + 4)
        count = (descriptor >> 16) & 31
        declarations[index] = (at, source, descriptor, [word(at + 8 + j * 4) for j in range(count)])
        at += 8 + count * 4
    selected = set()
    pending = [6, 7]
    while pending:
        index = pending.pop()
        if index in selected:
            continue
        selected.add(index)
        _, source, _, gates = declarations[index]
        pending.extend(ref & 255 for ref in [source, *gates] if ref >> 29 == 0)
    check(sorted(selected), [6, 7, 8, 52, 53, 113], 'pitch declaration cone')
    scalars = {}
    volume = struct.unpack('>602i', tables['volume'])
    for index in sorted(selected):
        at, source, descriptor, gates = declarations[index]
        pair, meta, run, curve, refs, authored = [alloc(n) for n in [8, 16, 12, 16, 4 * len(gates), 8]]
        magnitude = abs(struct.unpack('>h', struct.pack('>H', descriptor & 65535))[0])
        depth = 32767 - (0 if magnitude // 602 > 15 else volume[601 - magnitude % 602] >> (magnitude // 602))
        for address, value in [(pair, meta), (pair + 4, run), (meta, authored),
                               (meta + 8, max(0, struct.unpack('>h', struct.pack('>H', descriptor & 65535))[0])),
                               (meta + 12, depth), (run, curve), (run + 4, refs if gates else 0),
                               (curve + 12, 32767)]:
            m.w32(address, value)
        m.write(authored + 4, bytes([len(gates), 0, 0, 0]))
        scalars[index] = (pair, run, curve, refs)
    for index in selected:
        for j, ref in enumerate(declarations[index][3]):
            m.w32(scalars[index][3] + j * 4, scalars[ref & 255][2] + 12)

    outputs = alloc(20 * 8)
    buffers = [alloc(64) for _ in range(10)]
    for i in range(10):
        for j, (record, mapping, refs) in enumerate([(0x454c, 0x45d8, [6, 7]), (0x459c, 0x4614, [])]):
            meta, run, levels, mods = [alloc(n) for n in [16, 20, 8, 4]]
            pair = outputs + (i * 2 + j) * 8
            for address, value in [(pair, meta), (pair + 4, run), (meta, image + record),
                                   (meta + 8, (1 << 16) | len(refs)), (meta + 12, image + mapping),
                                   (run + 4, levels if refs else 0), (run + 12, mods),
                                   (run + 16, buffers[i]), (mods, nodes[i][0])]:
                m.w32(address, value)
            for k, ref in enumerate(refs):
                m.w32(levels + k * 4, scalars[ref][1] + 8)
    m.w32(evaluator + 0x1dc, 20)
    m.w32(evaluator + 0x1c0, outputs)
    obj, owner = alloc(16), alloc(16)
    m.w32(obj + 12, owner)

    harness = r'''
use std::io::{self, BufRead};
use skate_core::audio::{collision_mix::CollisionMix, curve::CurveTable, scalar::ScalarTables,
    scalar_program::ScalarProgram, input::WordInputs, pitch::{PitchTables,collision_pitch}, modulation::mxb};
fn main() {
 let dir=std::path::PathBuf::from(std::env::args().nth(1).unwrap());
 let read=|name| std::fs::read(dir.join(name)).unwrap();
 let data=read("MixMapSK8.mxb");
 let tables=ScalarTables::from_be_bytes(&read("log.bin"),&read("volume.bin")).unwrap();
 let curves=CurveTable::from_be_bytes(&read("curve.bin")).unwrap();
 let pitch=PitchTables::from_be_bytes(&read("pitch_semitones.bin"),&read("pitch_cents.bin")).unwrap();
 let mut counts=vec![0;14];counts[0]=1;counts[1]=2;counts[3]=10;
 let mut mix=CollisionMix::from_mxb(&data,&counts,&tables).unwrap();
 let selected=[6,7,8,52,53,113];
 let mut scalar=ScalarProgram::from_mxb_selected(&data,&counts,&tables,&selected.map(|n|(0,n))).unwrap();
 let mut nodes=mxb::load(&data,&counts,&[(3,0)]).unwrap();
 let mut inputs=WordInputs::default();
 for owner in mix.required_inputs(){inputs.attach(owner).unwrap();}
 for line in io::stdin().lock().lines(){
  let line=line.unwrap();let v:Vec<u32>=line.split_whitespace().map(|s|s.parse().unwrap()).collect();
  for (i,key) in [0x40000024,0x40000025,0x400000c0,0x40000071,0x400000c3].iter().enumerate(){inputs.set(*key,v[i]);}
  let external:Vec<u16>=scalar.external_inputs().iter().map(|id|inputs.get(*id).unwrap() as u16).collect();
  scalar.advance(&external,&curves,&tables).unwrap();
  let mut result:Vec<i64>=selected.iter().map(|n|scalar.level(0,0,*n).unwrap() as i64).collect();
  for i in 0..10 {let mut words:[u32;16]=v[5+i*17..21+i*17].try_into().unwrap();
   let s=nodes[i].node.advance(0,0,&mut words,&curves,&tables).unwrap();
   result.extend([s.phase as i64,s.linear as i64,s.log as i64,nodes[i].node.pitch() as i64,words[15] as i64]);
   *inputs.words_mut(0x60030000|((i as u32)<<11)).unwrap()=v[5+i*17..21+i*17].try_into().unwrap();
   if v[21+i*17]!=0{mix.activate(i)}else{mix.reset(i)}
  }
  mix.advance(1.0/30.0,&mut inputs,&curves,&tables).unwrap();
  for i in 0..10{let w=mix.words(i).unwrap(); result.extend([(w[0]>>16) as i16 as i64,w[11] as i16 as i64]);
   for (authored,slot) in [(3796,1),(4096,1),(3096,1),(4096,22)] {
    let p=collision_pitch(authored,Some(w),slot,&pitch).unwrap();result.extend([p.controller_pitch as i64,p.combined_pitch as i64,p.ratio.to_bits() as i64]);
   }
  }
  let p=collision_pitch(3796,None,1,&pitch).unwrap();result.push(p.ratio.to_bits() as i64);
  println!("{}",result.iter().map(|v|v.to_string()).collect::<Vec<_>>().join(" "));
 }
}
'''
    target = root / 'target'
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='6')
    subprocess.run(['cargo', 'build', '-p', 'skate-core', '--lib', '--offline'], cwd=root, env=env, check=True)
    rng = random.Random(0x8292a458)
    fixtures, expected = [], []
    for frame in range(160):
        profiles = [[32767, 0, 0, 0, 0]] * 4 + [
            [16383, 0, 0, 0, 0], [16383, 0, 0, 0, 0],
            [16383, 0, 0, 0, 32767], [16383, 0, 0, 0, 32767],
            [16383, 0, 0, 0, 32767], [16383, 0, 0, 0, 32767],
            [8191, 0, 0, 0, 32767], [8191, 0, 0, 0, 32767],
            [16383, 0, 32767, 0, 0], [16383, 0, 32767, 0, 0],
        ]
        values = profiles[frame] if frame < len(profiles) else [rng.randrange(32768) for _ in range(5)]
        external = dict(zip([0x48000024,0x49000025,0x480000c0,0x48000071,0x480000c3], values))
        row = list(values)
        # Native shared curves, in selector-bin/first-occurrence order.
        for index in sorted(selected, key=lambda n: ((declarations[n][1] >> 24) & 15, n)):
            source = declarations[index][1]
            m.r[3] = m.u32(scalars[source & 255][2] + 12) if source >> 29 == 0 else external[source]
            m.r[4] = (source >> 24) & 15
            call(0x82923f18)
            m.w32(scalars[index][2] + 12, m.r[3])
        for index in sorted(selected):
            m.r[4], m.r[26], m.r[31] = scalars[index][0], (-10000) & 0xffffffff, 0x82fbbd98
            call(0x82928140, 0x829283a4)
        result = [m.s32(m.u32(scalars[i][1] + 8)) for i in sorted(selected)]
        for i, (_, _, _, inputs) in enumerate(nodes):
            words = [0] * 16
            words[1] = f32_bits(5.0 if frame < 14 else rng.choice([0.,1.,5.,35.,50.,60.]))
            words[3] = 0 if frame < 14 else rng.choice([0,16383,16384,32768,49152,65535])
            words[14] = f32_bits(0.0 if frame < 14 else rng.choice([-500.,-340.,-339.,-100.,0.,1.,100.,800.,100000.]))
            words[15] = 1 if frame < 14 else rng.choice([0,1,0x40000001,0x80000001])
            enabled = 1 if frame < 14 else rng.randrange(2)
            row.extend([*words, enabled])
            for j, value in enumerate(words):m.w32(inputs + j * 4, value)
            m.w32(buffers[i] + 60, enabled)
        m.r[3] = evaluator
        call(0x82929f00)
        for _, _, run, inputs in nodes:
            result.extend([m.u32(run+8),m.u32(run+16),m.s32(m.u32(run+12)),m.s32(m.u32(run+20)),m.u32(inputs+60)])
        m.r[28],m.r[27],m.r[26],m.r[4] = evaluator,0,(-10000)&0xffffffff,0xffff0000
        call(0x82928470,0x82928520)
        m.r[3] = evaluator
        call(0x82928530)
        for buffer in buffers:
            cents = [(m.u32(buffer)>>16)&65535,m.u32(buffer+44)&65535]
            result.extend(v-65536 if v&32768 else v for v in cents)
            m.w32(owner+12,buffer)
            for authored, slot in [(3796,1),(4096,1),(3096,1),(4096,22)]:
                m.r[3],m.r[4] = obj,slot
                call(0x8249d0a8)
                controller = m.s32(m.r[3])
                m.set_fpr('f2',float(controller));m.set_fpr('f3',float(authored))
                m.set_fpr('f30',1/4096);m.set_fpr('f1',0.);m.set_fpr('f31',0.)
                call(0x824c03d4,0x824c040c)
                result.extend([controller,m.s32(m.u32(0x408000+0x7c)),m.u32(0x408000+0xb4)])
        m.w32(obj+12,0);m.r[3],m.r[4]=obj,1
        call(0x8249d0a8)
        check(m.r[3],0,'absent controller')
        m.w32(obj+12,owner)
        result.append(0)
        fixtures.append(' '.join(map(str,row)))
        expected.append(result)
    with tempfile.TemporaryDirectory(prefix='collision-pitch-') as temp:
        temp=Path(temp)
        (temp/collision_mix.PROGRAM[0]).write_bytes(data)
        for name,blob in tables.items():(temp/f'{name}.bin').write_bytes(blob)
        rust,binary=temp/'main.rs',temp/'verify'
        rust.write_text(harness)
        subprocess.run([shutil.which('rustc'),'--edition=2024','-O',str(rust),'--extern',f'skate_core={target}/debug/libskate_core.rlib','-L',f'dependency={target}/debug/deps','-o',str(binary)],check=True)
        output=subprocess.run([str(binary),str(temp)],input='\n'.join(fixtures)+'\n',text=True,capture_output=True,check=True)
    actual=[list(map(int,line.split())) for line in output.stdout.splitlines()]
    check(len(actual),len(expected),'frame count')
    for frame,(a,e) in enumerate(zip(actual,expected)):check(a,e,f'frame {frame}')
    print('160 phases: 960 declaration levels, 1600 modulation states/flags, 3200 packed pitch slots, 6400 voice consumers and 160 absent controllers matched')
    print('Free-skate first phase slots:', expected[0][56:58], 'settled:', expected[3][56:58])
    print('Half-speed, HOM, latched alternate, treatment, fresh alternate:', [expected[i][56:58] for i in [5,7,9,11,13]])


if __name__ == '__main__':
    main()
