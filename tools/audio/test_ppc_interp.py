import math
import struct
import unittest

from tools.native_replay.ppc_interp import Machine, Unsupported
from tools.native_replay.xex_image import IMAGE_BASE

BLR = 0x4E800020


def d_form(opcode, rt, ra, value):
    return (opcode << 26) | (rt << 21) | (ra << 16) | (value & 0xFFFF)


def x_form(rs, ra, rb, xo):
    return (31 << 26) | (rs << 21) | (ra << 16) | (rb << 11) | (xo << 1)


def a_form(xo, frt, fra, frc, frb):
    return (59 << 26) | (frt << 21) | (fra << 16) | (frb << 11) | (frc << 6) | (xo << 1)


def machine(*words):
    """A machine whose image holds synthetic instructions, not game code."""
    return Machine(struct.pack(f'>{len(words)}I', *words))


def run(words, ctr=0, **registers):
    m = machine(*words)
    m.ctr = ctr
    for name, value in registers.items():
        if name.startswith('f'):
            m.set_fpr(name, value)
        else:
            m.r[int(name[1:])] = value
    m.call(IMAGE_BASE)
    return m


class MachineTests(unittest.TestCase):
    def test_runs_until_the_call_returns(self):
        m = run([d_form(14, 3, 0, 5), d_form(14, 3, 3, 7), BLR])  # li r3,5; addi r3,r3,7
        self.assertEqual((m.r[3], m.steps), (12, 3))

    def test_unsigned_word_division(self):
        divwu = x_form(3, 4, 5, 459)  # divwu r3,r4,r5
        self.assertEqual(run([divwu, BLR], r4=0xFFFFFFFF, r5=2).r[3], 0x7FFFFFFF)
        with self.assertRaises(Unsupported):
            run([divwu, BLR], r4=1, r5=0)

    def test_word_shifts_clear_from_32_and_sraw_sign_extends(self):
        slw, srw, sraw = (x_form(4, 3, 5, xo) for xo in (24, 536, 792))  # op r3,r4,r5
        for shift, left, right in [(4, 0x23456790, 0x01234567), (31, 0x80000000, 0),
                                   (32, 0, 0), (63, 0, 0)]:
            self.assertEqual(run([slw, BLR], r4=0x12345679, r5=shift).r[3], left)
            self.assertEqual(run([srw, BLR], r4=0x12345679, r5=shift).r[3], right)
        for value, shift, result, carry in [(0x80000010, 4, 0xFFFFFFFFF8000001, 0),
                                            (0x80000010, 5, 0xFFFFFFFFFC000000, 1),
                                            (0x80000010, 40, 0xFFFFFFFFFFFFFFFF, 1),
                                            (0x7FFFFFFF, 40, 0, 0)]:
            m = run([sraw, BLR], r4=value, r5=shift)
            self.assertEqual((m.r[3], m.ca), (result, carry))

    def test_rotate_and_decrement_branch(self):
        self.assertEqual(run([0x5483403E, BLR], r4=0x12345678).r[3], 0x34567812)  # rotlwi r3,r4,8
        program = [(16 << 26) | (18 << 21) | 8, d_form(14, 3, 0, 1), BLR]  # bdz past li r3,1
        self.assertEqual((run(program, ctr=1).r[3], run(program, ctr=2).r[3]), (0, 1))

    def test_fused_multiply_subtract_rounds_once(self):
        a = 1 + 2**-12  # a * a - 1 needs one more bit than an unfused product keeps
        for xo, expected in [(28, 2**-11 + 2**-24), (30, -(2**-11 + 2**-24))]:  # fmsubs, fnmsubs
            m = run([a_form(xo, 1, 2, 3, 4), BLR], f2=a, f3=a, f4=1.0)
            self.assertEqual(m.fpr('f1'), expected)
        with self.assertRaises(Unsupported):
            run([a_form(28, 1, 2, 3, 4), BLR], f2=math.inf, f3=1.0, f4=1.0)

    def test_fctidz_truncates_and_saturates(self):
        fctidz = (63 << 26) | (1 << 21) | (2 << 11) | (815 << 1)  # fctidz f1,f2
        for value, raw in [(-2.5, 0xFFFFFFFFFFFFFFFE), (2.0**70, 0x7FFFFFFFFFFFFFFF),
                           (-2.0**70, 0x8000000000000000), (math.nan, 0x8000000000000000)]:
            self.assertEqual(run([fctidz, BLR], f2=value).f[1], raw)

    def test_twi_traps_on_its_condition(self):
        twi = d_form(3, 6, 3, 0)  # twi 6,r3,0: trap when r3 <= 0 unsigned
        run([twi, BLR], r3=1)
        with self.assertRaisesRegex(RuntimeError, 'Native trap at 82000000'):
            run([twi, BLR], r3=0)

    def test_dcbzl_clears_its_128_byte_line(self):
        m = machine(x_form(1, 0, 4, 1014), BLR)  # dcbzl 0,r4 (Xenon dcbz128)
        m.write(0x100000, b'\xff' * 0x200)
        m.r[4] = 0x100085
        m.call(IMAGE_BASE)
        self.assertEqual(m.read(0x100000, 0x200), b'\xff' * 0x80 + bytes(0x80) + b'\xff' * 0x100)
        with self.assertRaises(Unsupported):
            run([x_form(0, 0, 4, 1014), BLR], r4=0x100000)  # plain dcbz is not modelled

    def test_vector_register_move(self):
        m = machine(0x10221484, BLR)  # vmr v1,v2
        m.v[2] = bytes(range(16))
        m.call(IMAGE_BASE)
        self.assertEqual(m.v[1], bytes(range(16)))

    def test_memory_maps_the_image_writes_and_writable_ranges_only(self):
        lwz = d_form(32, 3, 4, 0)  # lwz r3,0(r4)
        self.assertEqual(run([lwz, BLR], r4=IMAGE_BASE).r[3], lwz)
        with self.assertRaises(Unsupported):
            run([lwz, BLR], r4=0x100000)
        m = machine(lwz, BLR)
        m.writable = [(0x100000, 0x100004)]
        m.r[4] = 0x100000
        m.call(IMAGE_BASE)
        self.assertEqual(m.r[3], 0)
        m.w32(0x100000, 0xDEADBEEF)
        m.call(IMAGE_BASE)
        self.assertEqual(m.r[3], 0xDEADBEEF)

    def test_undecodable_and_unsupported_instructions_raise(self):
        with self.assertRaises(Unsupported):
            run([0, BLR])
        with self.assertRaises(Unsupported):
            run([d_form(3, 4, 3, 0), BLR])  # tweqi has no replay semantics here


if __name__ == '__main__':
    unittest.main()
