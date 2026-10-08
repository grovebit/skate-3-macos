"""Replay AEMS selection and CRT RNG against a research reference, not a Rust port.

Replays the owned routines from your own default.xex. The TLS accessor is
relocated; pow(2, n), n=1..16, is an explicit exact dependency hook. No RNG,
selection branches, state writes or floating selection arithmetic are hooked.
"""
import argparse
from pathlib import Path
import struct

from tools.native_replay import xex_image
from tools.native_replay.ppc_interp import Machine


def rng_step(state):
    state = (state * 214013 + 2531011) & 0xffffffff
    return state, (state >> 16) & 0x7fff


def select(count, mode, state, random_word):
    """8294F340, valid authored counts 1..32; state is an unsigned word."""
    if (not 1 <= count <= 32 or mode not in (0, 1, 2)
            or not 0 <= random_word <= 32767 or not 0 <= state <= 0xffffffff):
        raise ValueError('Outside verified selection domain')
    if count == 1:
        return 0, state
    if mode == 0:
        return int((random_word / 32768) * count), state
    if mode == 1:
        incremented = (state + 1) & 0xffffffff
        signed = incremented if incremented < 0x80000000 else incremented - 0x100000000
        quotient = abs(signed) // count * (-1 if signed < 0 else 1)
        remainder = signed - quotient * count
        return remainder & 255, remainder & 0xffffffff
    half, flag, mask = count // 2, state & 1, state >> 16
    size = half + (flag & count & 1)
    start = int((random_word / 32768) * (size + 1))
    for step in range(size):
        bit = (start + step) % size
        if mask & (1 << bit):
            result = flag * half + bit
            mask &= ~(1 << bit)
            if mask == 0:
                flag ^= 1
                mask = (1 << (half + (flag & count & 1))) - 1
            return result, (mask << 16) | flag
    return 0, state


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', type=Path, required=True,
                        help='your Skate 3 folder, with default.xex')
    args = parser.parse_args()
    image = xex_image.load(args.game)
    state_ptr, tls = 0x100000, 0x101000

    def machine():
        m = Machine(image)
        m.writable = [(0x100000, 0x110000), (0x200000, 0x210000)]
        m.r[1] = 0x208000
        m.hooks[0x82f2d020] = lambda m: m.r.__setitem__(3, tls)
        def power(m):
            exponent = m.fpr('f2')
            if m.fpr('f1') != 2 or exponent != int(exponent) or not 1 <= exponent <= 16:
                raise RuntimeError('Unexpected pow dependency')
            m.set_fpr('f1', float(2 ** int(exponent)))
        m.hooks[0x82f279f0] = power
        return m

    total = 0
    for count in range(1, 33):
        for mode in range(3):
            m = machine()
            state = 0x10000
            seed = 1
            m.w32(state_ptr, state)
            m.w32(tls + 0x14, seed)
            for _ in range(128):
                consumes = count != 1 and mode != 1
                next_seed, word = rng_step(seed)
                expected, state = select(count, mode, state, word)
                if consumes:
                    seed = next_seed
                m.r[3], m.r[4], m.r[5] = count, mode, state_ptr
                m.steps = 0
                m.call(0x8294f340)
                actual = (m.r[3], m.u32(state_ptr), m.u32(tls + 0x14))
                if actual != (expected, state, seed):
                    raise RuntimeError(f'count={count} mode={mode} fixture={total}: {actual} != {(expected, state, seed)}')
                total += 1
    for count in range(1, 33):
        for state in (0, 0x7ffffffe, 0x7fffffff, 0x80000000, 0xfffffffe, 0xffffffff):
            m = machine()
            m.w32(state_ptr, state)
            m.r[3], m.r[4], m.r[5] = count, 1, state_ptr
            m.call(0x8294f340)
            if (m.r[3], m.u32(state_ptr)) != select(count, 1, state, 0):
                raise RuntimeError('Sequential width boundary mismatch')
            total += 1
    m = machine()
    seed = 0xffffffff
    m.r[3] = seed
    m.call(0x82f23e70)
    if m.u32(tls + 0x14) != seed:
        raise RuntimeError('Seed setter mismatch')
    for _ in range(4096):
        seed, word = rng_step(seed)
        m.steps = 0
        m.call(0x82f23ea0)
        if (m.r[3], m.u32(tls + 0x14)) != (word, seed):
            raise RuntimeError('RNG mismatch')
    # Replay the complete layer-selection loop, including its rejection
    # continuation. Only the allocator and TLS lookup are external hooks.
    activations = 0
    for probability in (0., .5, 1.):
        for seed in (1, 17, 0x12345678, 0xffffffff,
                     (-2531011 * pow(214013, -1, 1 << 32)) & 0xffffffff):
            m = machine()
            event, leaf, layer, choice = 0x106000, 0x106100, 0x106200, 0x10620c
            m.w32(event + 0x54, leaf)
            m.write(leaf + 7, bytes([3]))
            m.w32(leaf + 0x20, layer)
            for i in range(3):
                m.w32(layer + i * 84, choice + i * 84)
                m.w32(layer + i * 84 + 4, 0x10000)
                m.write(layer + i * 84 + 8, bytes([1, 1]))
                m.wf32(choice + i * 84 + 0x40, probability)
            m.w32(0x8307360c, 0x102000)
            m.w32(0x102004, 0x103000)
            m.w32(0x103000 + 0xc1b4, 0x104000)
            m.w32(0x82fda420, 0)
            m.w32(0x10400c, 0x105000)
            m.w32(0x105008, 0x101100)
            allocated = []
            def allocate(m):
                pointer = 0x107000 + len(allocated) * 0x100
                allocated.append(pointer)
                m.r[3] = pointer
            m.hooks[0x101100] = allocate
            m.w32(tls + 0x14, seed)
            expected = []
            cursor = choice
            for _ in range(3):
                seed, word = rng_step(seed)
                if word / 32768 <= probability:
                    expected.append(cursor)
                    cursor += 84
                else:
                    expected.append(None)
            m.r[3], m.r[4] = event, 0x108000
            m.call(0x8294dd38)
            actual = [m.u32(p + 0x58) if (p := m.u32(event + 4 + i * 4)) else None
                      for i in range(3)]
            if actual != expected or m.u32(tls + 0x14) != seed:
                raise RuntimeError(f'Layer activation mismatch: {actual} != {expected}')
            activations += 1
    def f32(value):
        return struct.unpack('>f', struct.pack('>f', value))[0]

    def bits(value):
        return struct.unpack('>I', struct.pack('>f', value))[0]

    starts = 0
    for gain_range in (.5, 1., 2.):
        for delay_range in (0., .125):
            for word in (0, 16384, 32767):
                m = machine()
                layer, choice, params = 0x106000, 0x106100, 0x106200
                m.w32(layer + 0x58, choice)
                m.wf32(choice + 0x2c, gain_range)
                m.wf32(choice + 8, 1.)
                m.wf32(choice + 0x30, .25)
                m.wf32(choice + 0x14, .125)
                m.wf32(choice + 0x34, delay_range)
                seed = (((word << 16) - 2531011) * pow(214013, -1, 1 << 32)) & 0xffffffff
                m.w32(tls + 0x14, seed)
                seed, first = rng_step(seed)
                x = first / 16384 - 1
                gain = f32(1 + x * (1 / gain_range - 1) if x > 0
                           else 1 + x * (1 - gain_range))
                seed, second = rng_step(seed)
                rate = f32(1 + (second / 32768) * .25)
                delay = .125
                if delay_range != 0:
                    seed, third = rng_step(seed)
                    delay = f32(delay + (third / 32768) * delay_range)
                m.r[3], m.r[4], m.r[5] = layer, 0x106300, params
                m.call(0x8294e230)
                actual = tuple(m.u32(layer + offset) for offset in (0x44, 0x40, 0x48, 0x50))
                expected = tuple(map(bits, (gain, rate, rate, delay)))
                if actual != expected or m.u32(tls + 0x14) != seed or m.u32(layer + 0x4c) != 1:
                    raise RuntimeError('Layer start gain/rate/delay or RNG ordering mismatch')
                starts += 1
    delays = 0
    for delay in (0., .125, .25, -.125):
        for delta in (0., .0625, .125, .25):
            m = machine()
            layer, choice, params = 0x106000, 0x106100, 0x106200
            m.w32(layer + 0x58, choice)
            m.w32(layer + 0x4c, 1)
            m.wf32(layer + 0x50, delay)
            m.wf32(params + 0xc, delta)
            queued = []
            m.hooks[0x8294dbd0] = lambda m: queued.append(m.r[4])
            m.r[3], m.r[4], m.r[5] = layer, 0x106300, params
            m.call(0x8294edc8)
            remaining = delay - delta if delay > 0 else delay
            if (m.u32(layer + 0x50) != bits(remaining)
                    or bool(queued) != (delay > 0 and remaining <= 0)):
                raise RuntimeError('Delay decrement/expiry mismatch')
            delays += 1
    envelopes = 0
    for phase in (-1., 0., .125, .1875, .25, .75, .875, 1., 2.):
        m = machine()
        layer, choice, params = 0x106000, 0x106100, 0x106200
        m.w32(layer + 0x58, choice)
        m.wf32(layer + 0x54, phase)
        for offset, value in ((0x18, .125), (0x1c, 1.), (0x20, .25), (0x24, .75)):
            m.wf32(choice + offset, value)
        m.write(choice + 0x28, bytes([0x42]))  # Both branches use the LOW nibble.
        m.wf32(params, .5)
        m.r[3], m.r[4] = layer, params
        m.call(0x8294f258)
        envelope = ((phase - .125) / .125 if phase < .25 else
                    1 - (phase - .75) / .25 if phase > .75 else 1)
        if m.u32(params) != bits(.5 * min(1., max(0., envelope))):
            raise RuntimeError('Linear envelope boundary mismatch')
        envelopes += 1
    print(f'{total} native/reference selections, 4096 RNG transitions, seed setter, '
          f'{activations} activation loops, {starts} start controls, {delays} delay boundaries '
          f'and {envelopes} linear envelope cases matched; exact pow dependency hooked')



if __name__ == '__main__':
    main()
