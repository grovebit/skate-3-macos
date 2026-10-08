"""Instruction-subset PowerPC interpreter for replaying original routines.

Verifiers execute functions from the mapped image of your own base-disc
default.xex (`xex_image.load`) and compare the results with production Rust;
no game bytes are stored in the repository. Calls between executed functions,
including the compiler's save/restore helpers, run natively, so they keep the
original stack discipline. Unsupported instructions and reads of unmapped
memory raise instead of producing guessed values.

Vector lanes round to binary32 per operation; fused multiply-adds round once.
Estimates (vrefp, vrsqrtefp) and vmsum3fp128 use the same host arithmetic as
the port's native_arithmetic module, so replays verify lanes, permutes,
operation order and branches, not Xenon estimate bits.
"""
import math
import re
import struct
from fractions import Fraction

from capstone import CS_ARCH_PPC, CS_MODE_32, CS_MODE_BIG_ENDIAN, Cs

from tools.owned_game.ppc_vmx import decode as vmx_decode
from tools.native_replay.xex_image import IMAGE_BASE

M64 = (1 << 64) - 1
M32 = (1 << 32) - 1
LT, GT, EQ, SO = 8, 4, 2, 1
_DISASSEMBLER = Cs(CS_ARCH_PPC, CS_MODE_32 | CS_MODE_BIG_ENDIAN)


def f32_bits(value):
    return struct.unpack('>I', struct.pack('>f', value))[0]


def to_f32(value):
    """Round a binary64 value to binary32 (round-to-nearest-even)."""
    return struct.unpack('>f', struct.pack('>f', value))[0]


def d2bits(value):
    return struct.unpack('>Q', struct.pack('>d', value))[0]


def bits2d(raw):
    return struct.unpack('>d', struct.pack('>Q', raw & M64))[0]


def exact_f32(fraction):
    """Round an exact rational to binary32, nearest-even, finite inputs only."""
    if fraction == 0:
        return 0.0
    sign = -1 if fraction < 0 else 1
    x = abs(fraction)
    e = math.floor(math.log2(x.numerator) - math.log2(x.denominator))
    while Fraction(2) ** e > x: e -= 1
    while Fraction(2) ** (e + 1) <= x: e += 1
    e = max(e, -126)
    scale = Fraction(2) ** (e - 23)
    q, r = divmod(x / scale, 1)
    q = int(q)
    if r > Fraction(1, 2) or (r == Fraction(1, 2) and q & 1):
        q += 1
    return sign * float(q * scale)


def _lanes(b): return list(struct.unpack('>4f', b))
def _words(b): return list(struct.unpack('>4I', b))
def _pack(fs): return struct.pack('>4f', *[to_f32(f) for f in fs])
def _packw(ws): return struct.pack('>4I', *[w & M32 for w in ws])


def _fused(a, b, c, negate=False):
    if not all(map(math.isfinite, (a, b, c))):
        # Unused lanes may carry inf/NaN (e.g. estimates of zero lanes);
        # they propagate without affecting the published lane 0.
        with_inf = a * b + c if not (math.isinf(a * b) and math.isinf(c) and (a * b) != c) else math.nan
        return -with_inf if negate else with_inf
    v = Fraction(a) * Fraction(b) + Fraction(c)
    return exact_f32(-v if negate else v)


class Unsupported(Exception):
    pass


class Machine:
    def __init__(self, image):
        self.image = image
        self.mem = {}
        self.r = [0] * 32
        self.f = [0] * 32
        self.v = [bytes(16)] * 128
        self.cr = [0] * 8
        self.ca = 0
        self.lr = 0
        self.ctr = 0
        self.hooks = {}
        self.cache = {}
        self.steps = 0
        self.writable = []  # (start, end) ranges that are valid without prior writes

    # Memory -----------------------------------------------------------
    def read8(self, a):
        a &= M32
        if a in self.mem:
            return self.mem[a]
        if IMAGE_BASE <= a < IMAGE_BASE + len(self.image):
            return self.image[a - IMAGE_BASE]
        for start, end in self.writable:
            if start <= a < end:
                return 0
        raise Unsupported(f'read of unmapped {a:08x}')

    def read(self, a, n):
        return bytes(self.read8(a + i) for i in range(n))

    def write(self, a, data):
        for i, byte in enumerate(data):
            self.mem[(a + i) & M32] = byte

    def u32(self, a):
        return struct.unpack('>I', self.read(a, 4))[0]

    def w32(self, a, v):
        self.write(a, struct.pack('>I', v & M32))

    def wf32(self, a, v):
        self.write(a, struct.pack('>f', v))

    # Registers --------------------------------------------------------
    def gpr(self, name):
        return self.r[int(name[1:])]

    def set_gpr(self, name, value):
        self.r[int(name[1:])] = value & M64

    def fpr(self, name):
        return bits2d(self.f[int(name[1:])])

    def set_fpr(self, name, value):
        self.f[int(name[1:])] = d2bits(value)

    def vr(self, name):
        return self.v[int(name[1:])]

    def setv(self, name, data):
        self.v[int(name[1:])] = data

    @staticmethod
    def s32(v):
        v &= M32
        return v - (1 << 32) if v & 0x80000000 else v

    @staticmethod
    def s64(v):
        v &= M64
        return v - (1 << 64) if v & (1 << 63) else v

    def ea(self, op):
        off, base = re.fullmatch(r'(-?(?:0x[0-9a-f]+|\d+))\((r\d+)\)', op).groups()
        base_value = 0 if base == 'r0' else self.gpr(base)
        return (base_value + int(off, 0)) & M64

    def crfield(self, name):
        return int(name[2:]) if name.startswith('cr') else 0

    def compare(self, field, a, b):
        self.cr[field] = (LT if a < b else GT if a > b else EQ)

    def fcompare(self, field, a, b):
        if math.isnan(a) or math.isnan(b):
            self.cr[field] = SO  # unordered
        else:
            self.compare(field, a, b)

    # Decoding ---------------------------------------------------------
    def decode(self, pc):
        if pc in self.cache:
            return self.cache[pc]
        raw = self.read(pc, 4)
        word = struct.unpack('>I', raw)[0]
        vmx = vmx_decode(word)
        if vmx is not None:
            mn, ops = vmx[0], vmx[1]
        else:
            got = list(_DISASSEMBLER.disasm_lite(raw, pc))
            if not got:
                raise Unsupported(f'undecodable {pc:08x} {word:08x}')
            mn, ops = got[0][2], got[0][3]
        parsed = (mn, [o.strip() for o in ops.split(',')] if ops else [], word)
        self.cache[pc] = parsed
        return parsed

    # Execution --------------------------------------------------------
    def call(self, entry, sentinel=0x7FFFFFF0, limit=2_000_000):
        """Run from entry until it returns to `sentinel`."""
        self.lr = sentinel
        pc = entry
        while pc != sentinel:
            if pc in self.hooks:
                self.hooks[pc](self)
                pc = self.lr
                continue
            self.steps += 1
            if self.steps > limit:
                raise Unsupported('step limit')
            pc = self.step(pc)

    def branch_condition(self, mn, ops):
        """Return (taken, target_operand_index) for simplified bc mnemonics."""
        conds = {'eq': (EQ, True), 'ne': (EQ, False), 'lt': (LT, True), 'ge': (LT, False),
                 'gt': (GT, True), 'le': (GT, False), 'un': (SO, True), 'nu': (SO, False)}
        m = re.fullmatch(r'b(eq|ne|lt|ge|gt|le|un|nu)(lr|ctr)?(l)?', mn)
        if not m:
            return None
        bit, sense = conds[m[1]]
        if ops and ops[0].startswith('cr'):
            field = int(ops[0][2:]); rest = ops[1:]
        else:
            field = 0; rest = ops
        value = bool(self.cr[field] & bit)
        return value == sense, m[2], rest

    def step(self, pc):
        mn, ops, word = self.decode(pc)
        if mn.startswith('v') or mn in ('lvlx128', 'mfocrf'):
            self.vector(pc, mn, ops)
            return pc + 4
        nxt = pc + 4
        g, sg = self.gpr, self.set_gpr
        r = mn
        if r in ('lwz', 'lwzu', 'lbz', 'lbzu', 'lhz', 'ld', 'ldu', 'lha'):
            a = self.ea(ops[1])
            size = {'lwz': 4, 'lwzu': 4, 'lbz': 1, 'lbzu': 1, 'lhz': 2, 'ld': 8, 'ldu': 8, 'lha': 2}[r]
            v = int.from_bytes(self.read(a, size), 'big')
            if r == 'lha' and v & 0x8000: v -= 0x10000
            sg(ops[0], v)
            if r.endswith('u'): sg(re.search(r'\((r\d+)\)', ops[1])[1], a)
        elif r in ('lwzx', 'lbzx', 'lhzx', 'ldx'):
            a = ((0 if ops[1] == 'r0' else g(ops[1])) + g(ops[2])) & M64
            size = {'lwzx': 4, 'lbzx': 1, 'lhzx': 2, 'ldx': 8}[r]
            sg(ops[0], int.from_bytes(self.read(a, size), 'big'))
        elif r in ('stw', 'stwu', 'stb', 'stbu', 'sth', 'std', 'stdu'):
            a = self.ea(ops[1])
            size = {'stw': 4, 'stwu': 4, 'stb': 1, 'stbu': 1, 'sth': 2, 'std': 8, 'stdu': 8}[r]
            self.write(a, (g(ops[0]) & ((1 << (8 * size)) - 1)).to_bytes(size, 'big'))
            if r.endswith('u'): sg(re.search(r'\((r\d+)\)', ops[1])[1], a)
        elif r in ('stwx', 'stbx', 'sthx', 'stdx'):
            a = ((0 if ops[1] == 'r0' else g(ops[1])) + g(ops[2])) & M64
            size = {'stwx': 4, 'stbx': 1, 'sthx': 2, 'stdx': 8}[r]
            self.write(a, (g(ops[0]) & ((1 << (8 * size)) - 1)).to_bytes(size, 'big'))
        elif r in ('lfs', 'lfsx', 'lfd', 'lfdx'):
            a = self.ea(ops[1]) if not r.endswith('x') else ((0 if ops[1] == 'r0' else g(ops[1])) + g(ops[2])) & M64
            if r.startswith('lfs'):
                self.set_fpr(ops[0], struct.unpack('>f', self.read(a, 4))[0])
            else:
                self.f[int(ops[0][1:])] = int.from_bytes(self.read(a, 8), 'big')
        elif r in ('stfs', 'stfsx', 'stfsu', 'stfd', 'stfdx'):
            a = self.ea(ops[1]) if not r.endswith('x') else ((0 if ops[1] == 'r0' else g(ops[1])) + g(ops[2])) & M64
            if r.startswith('stfs'):
                self.write(a, struct.pack('>f', to_f32(self.fpr(ops[0]))))
            else:
                self.write(a, self.f[int(ops[0][1:])].to_bytes(8, 'big'))
            if r.endswith('u'): sg(re.search(r'\((r\d+)\)', ops[1])[1], a)
        elif r in ('lvx128', 'lvx'):
            a = ((0 if ops[1] == 'r0' else g(ops[1])) + g(ops[2])) & ~15 & M64
            self.v[int(ops[0][1:])] = self.read(a, 16)
        elif r in ('stvx128', 'stvx'):
            a = ((0 if ops[1] == 'r0' else g(ops[1])) + g(ops[2])) & ~15 & M64
            self.write(a, self.v[int(ops[0][1:])])
        elif r == 'li': sg(ops[0], int(ops[1], 0))
        elif r == 'lis': sg(ops[0], int(ops[1], 0) << 16)
        elif r in ('dcbt', 'dcbtst'): pass  # Cache hints do not change architectural storage.
        elif r == 'dcbzl':
            # Xenon's dcbz128: zero the whole 128-byte cache line.
            a = ((0 if ops[0] in ('r0', '0') else g(ops[0])) + g(ops[1])) & M32
            self.write(a & ~127, bytes(128))
        elif r == 'mr': sg(ops[0], g(ops[1]))
        elif r == 'addi': sg(ops[0], (0 if ops[1] == 'r0' else g(ops[1])) + int(ops[2], 0))
        elif r == 'addis': sg(ops[0], (0 if ops[1] == 'r0' else g(ops[1])) + (int(ops[2], 0) << 16))
        elif r in ('add', 'add.'):
            v = g(ops[1]) + g(ops[2]); sg(ops[0], v)
            if r == 'add.': self.compare(0, self.s64(v), 0)
        elif r == 'addic':
            v = g(ops[1]) + (int(ops[2], 0) & M64); self.ca = int(v > M64); sg(ops[0], v)
        elif r == 'addic.':
            v = g(ops[1]) + (int(ops[2], 0) & M64); self.ca = int(v > M64); sg(ops[0], v)
            self.compare(0, self.s64(v), 0)
        elif r == 'subfc':
            v = (~g(ops[1]) & M64) + g(ops[2]) + 1; self.ca = int(v > M64); sg(ops[0], v)
        elif r == 'addze':
            v = g(ops[1]) + self.ca; self.ca = int(v > M64); sg(ops[0], v)
        elif r == 'eqv': sg(ops[0], ~(g(ops[1]) ^ g(ops[2])))
        elif r == 'subf': sg(ops[0], g(ops[2]) - g(ops[1]))
        elif r == 'subfe':
            v = (~g(ops[1]) & M64) + g(ops[2]) + self.ca; self.ca = int(v > M64); sg(ops[0], v)
        elif r == 'subfic':
            v = (int(ops[2], 0) & M64) + (~g(ops[1]) & M64) + 1; self.ca = int(v > M64); sg(ops[0], v)
        elif r == 'neg': sg(ops[0], -g(ops[1]))
        elif r == 'mulld': sg(ops[0], g(ops[1]) * g(ops[2]))
        elif r == 'mullw': sg(ops[0], self.s32(g(ops[1])) * self.s32(g(ops[2])))
        elif r == 'mulli': sg(ops[0], self.s64(g(ops[1])) * int(ops[2], 0))
        elif r == 'mulhw': sg(ops[0], (self.s32(g(ops[1])) * self.s32(g(ops[2]))) >> 32)
        elif r == 'divw':
            a, b_ = self.s32(g(ops[1])), self.s32(g(ops[2]))
            if b_ == 0 or (a == -2**31 and b_ == -1): raise Unsupported('divw domain')
            q = abs(a) // abs(b_); sg(ops[0], q if (a < 0) == (b_ < 0) else -q)
        elif r == 'divwu':
            b_ = g(ops[2]) & M32
            if b_ == 0: raise Unsupported('divwu domain')
            sg(ops[0], (g(ops[1]) & M32) // b_)
        elif r in ('srawi', 'srawi.'):
            v = self.s32(g(ops[1])); sh = int(ops[2], 0)
            self.ca = int(v < 0 and (v & ((1 << sh) - 1)) != 0); sg(ops[0], v >> sh)
            if r.endswith('.'): self.compare(0, self.s64(g(ops[0])), 0)
        elif r == 'sraw':
            v = self.s32(g(ops[1])); sh = g(ops[2]) & 63
            self.ca = int(v < 0 and (v & ((1 << sh) - 1)) != 0); sg(ops[0], v >> sh)
        elif r in ('slw', 'srw'):
            x = g(ops[1]) & M32; sh = g(ops[2]) & 63
            sg(ops[0], 0 if sh > 31 else ((x << sh) if r == 'slw' else (x >> sh)) & M32)
        elif r == 'slwi': sg(ops[0], (g(ops[1]) << int(ops[2], 0)) & M32)
        elif r == 'srwi': sg(ops[0], (g(ops[1]) & M32) >> int(ops[2], 0))
        elif r == 'clrldi': sg(ops[0], g(ops[1]) & ((1 << (64 - int(ops[2], 0))) - 1))
        elif r == 'clrlwi': sg(ops[0], g(ops[1]) & ((1 << (32 - int(ops[2], 0))) - 1))
        elif r == 'rotlwi':
            x = g(ops[1]) & M32; sh = int(ops[2], 0)
            sg(ops[0], ((x << sh) | (x >> (32 - sh))) & M32)
        elif r in ('rlwinm', 'rlwimi'):
            x = g(ops[1]) & M32; sh = int(ops[2], 0); mb = int(ops[3], 0); me = int(ops[4], 0)
            rot = ((x << sh) | (x >> (32 - sh))) & M32 if sh else x
            mask = sum(1 << (31 - i) for i in range(32) if (mb <= i <= me if mb <= me else i >= mb or i <= me))
            if r == 'rlwinm': sg(ops[0], rot & mask)
            else: sg(ops[0], (g(ops[0]) & ~mask & M32) | (rot & mask))
        elif r == 'rldimi':
            x = g(ops[1]); sh = int(ops[2], 0); mb = int(ops[3], 0); me = 63 - sh
            rot = ((x << sh) | (x >> (64 - sh))) & M64 if sh else x
            mask = sum(1 << (63 - i) for i in range(64) if mb <= i <= me)
            sg(ops[0], (g(ops[0]) & ~mask & M64) | (rot & mask))
        elif r == 'extsw': sg(ops[0], self.s32(g(ops[1])))
        elif r == 'extsh':
            v = g(ops[1]) & 0xFFFF; sg(ops[0], v - 0x10000 if v & 0x8000 else v)
        elif r == 'extsb':
            v = g(ops[1]) & 0xFF; sg(ops[0], v - 0x100 if v & 0x80 else v)
        elif r == 'cntlzw': sg(ops[0], 32 - (g(ops[1]) & M32).bit_length())
        elif r == 'or': sg(ops[0], g(ops[1]) | g(ops[2]))
        elif r == 'xori': sg(ops[0], g(ops[1]) ^ int(ops[2], 0))
        elif r == 'ori': sg(ops[0], g(ops[1]) | int(ops[2], 0))
        elif r == 'oris': sg(ops[0], g(ops[1]) | (int(ops[2], 0) << 16))
        elif r == 'and': sg(ops[0], g(ops[1]) & g(ops[2]))
        elif r == 'andc': sg(ops[0], g(ops[1]) & ~g(ops[2]))
        elif r == 'xor': sg(ops[0], g(ops[1]) ^ g(ops[2]))
        elif r in ('cmpwi', 'cmpw', 'cmplwi', 'cmplw', 'cmpdi', 'cmpd', 'cmpldi', 'cmpld'):
            if ops[0].startswith('cr'): field = int(ops[0][2:]); a_op, b_op = ops[1], ops[2]
            else: field = 0; a_op, b_op = ops[0], ops[1]
            a = g(a_op); b_ = int(b_op, 0) if not b_op.startswith('r') else g(b_op)
            if r in ('cmpwi', 'cmpw'): self.compare(field, self.s32(a), self.s32(b_) if b_op.startswith('r') else b_)
            elif r in ('cmplwi', 'cmplw'): self.compare(field, a & M32, (b_ & M32) if b_op.startswith('r') else b_ & 0xFFFF)
            elif r in ('cmpdi', 'cmpd'): self.compare(field, self.s64(a), self.s64(b_) if b_op.startswith('r') else b_)
            else: self.compare(field, a & M64, b_ & M64)
        elif r == 'twi':
            to, a, b_ = int(ops[0], 0), self.s32(g(ops[1])), int(ops[2], 0)
            if ((to & 16 and a < b_) or (to & 8 and a > b_) or (to & 4 and a == b_)
                    or (to & 2 and a & M32 < b_ & M32) or (to & 1 and a & M32 > b_ & M32)):
                raise RuntimeError(f'Native trap at {pc:08x}')
        elif r == 'mflr': sg(ops[0], self.lr)
        elif r == 'mtlr': self.lr = g(ops[0]) & M32
        elif r == 'mtctr': self.ctr = g(ops[0]) & M64
        elif r == 'mfctr': sg(ops[0], self.ctr)
        elif r == 'b': return int(ops[0], 0)
        elif r == 'bl': self.lr = nxt; return int(ops[0], 0)
        elif r == 'blr': return self.lr
        elif r == 'bctr': return self.ctr & M32
        elif r == 'bctrl': self.lr = nxt; return self.ctr & M32
        elif r in ('bdnz', 'bdz'):
            self.ctr = (self.ctr - 1) & M64
            return int(ops[0], 0) if (self.ctr != 0) == (r == 'bdnz') else nxt
        elif r in ('bdzf', 'bdnzf', 'bdzt', 'bdnzt'):
            self.ctr = (self.ctr - 1) & M64
            cond, target = ops[0], ops[1]
            m = re.fullmatch(r'(?:4\*cr(\d)\+)?(lt|gt|eq|so|un)', cond)
            field = int(m[1]) if m[1] else 0
            bit = {'lt': LT, 'gt': GT, 'eq': EQ, 'so': SO, 'un': SO}[m[2]]
            value = bool(self.cr[field] & bit)
            ctr_ok = (self.ctr == 0) if r.startswith('bdz') else (self.ctr != 0)
            want = r.endswith('t')
            return int(target, 0) if ctr_ok and value == want else nxt
        elif r == 'fsel':
            selected = ops[2] if self.fpr(ops[1]) >= 0.0 else ops[3]
            self.f[int(ops[0][1:])] = self.f[int(selected[1:])]
        elif r == 'fmr': self.f[int(ops[0][1:])] = self.f[int(ops[1][1:])]
        elif r == 'fneg': self.f[int(ops[0][1:])] = self.f[int(ops[1][1:])] ^ (1 << 63)
        elif r == 'fabs': self.f[int(ops[0][1:])] = self.f[int(ops[1][1:])] & ~(1 << 63)
        elif r in ('fadds', 'fsubs', 'fmuls', 'fdivs'):
            a, b_ = self.fpr(ops[1]), self.fpr(ops[2])
            if r == 'fdivs' and b_ == 0: raise Unsupported('fdivs by zero')
            v = {'fadds': lambda: a + b_, 'fsubs': lambda: a - b_, 'fmuls': lambda: a * b_, 'fdivs': lambda: a / b_}[r]()
            self.set_fpr(ops[0], to_f32(v))
        elif r in ('fmadds', 'fmsubs', 'fnmsubs'):
            # Assembly order frD, frA, frC, frB; one rounding of A*C+B or A*C-B.
            a, c, b_ = self.fpr(ops[1]), self.fpr(ops[2]), self.fpr(ops[3])
            if not all(map(math.isfinite, (a, c, b_))): raise Unsupported(f'{r} domain')
            v = Fraction(a) * Fraction(c) + (Fraction(b_) if r == 'fmadds' else -Fraction(b_))
            self.set_fpr(ops[0], exact_f32(-v if r == 'fnmsubs' else v))
        elif r == 'frsp': self.set_fpr(ops[0], to_f32(self.fpr(ops[1])))
        elif r == 'fcmpu':
            field = int(ops[0][2:]); self.fcompare(field, self.fpr(ops[1]), self.fpr(ops[2]))
        elif r == 'fctiwz':
            v = self.fpr(ops[1])
            if math.isnan(v): i = -2**31
            else: i = max(-2**31, min(2**31 - 1, int(v)))
            self.f[int(ops[0][1:])] = 0xFFF8000000000000 | (i & M32)
        elif r == 'fctidz':
            v = self.fpr(ops[1])
            if math.isnan(v): i = -2**63
            else: i = max(-2**63, min(2**63 - 1, int(v)))
            self.f[int(ops[0][1:])] = i & M64
        elif r == 'fcfid':
            self.set_fpr(ops[0], float(self.s64(self.f[int(ops[1][1:])])))
        else:
            cond = self.branch_condition(r, ops)
            if cond is None:
                raise Unsupported(f'{pc:08x} {mn} {ops}')
            taken, kind, rest = cond
            if not taken: return nxt
            if kind == 'lr': return self.lr
            if kind == 'ctr': return self.ctr & M32
            return int(rest[0], 0)
        return nxt

    def vector(self, pc, mn, ops):
        base = mn.rstrip('.')
        record = mn.endswith('.')
        V = self.vr
        if base in ('vaddfp128', 'vsubfp128', 'vmulfp128', 'vmaxfp128', 'vminfp128'):
            a, b = _lanes(V(ops[1])), _lanes(V(ops[2]))
            op = {'vaddfp128': lambda x, y: x + y, 'vsubfp128': lambda x, y: x - y,
                  'vmulfp128': lambda x, y: x * y, 'vmaxfp128': max, 'vminfp128': min}[base]
            self.setv(ops[0], _pack([op(x, y) for x, y in zip(a, b)]))
        elif base in ('vmaddfp', 'vnmsubfp'):
            # Assembly order vD, vA, vC, vB.
            a, c, b = _lanes(V(ops[1])), _lanes(V(ops[2])), _lanes(V(ops[3]))
            neg = base == 'vnmsubfp'
            self.setv(ops[0], _pack([_fused(x, y, -z if neg else z, neg) for x, y, z in zip(a, c, b)]))
        elif base in ('vmaddfp128', 'vnmsubfp128'):
            # Destructive: vD = (vA * vB) + vD, or -((vA * vB) - vD).
            a, b, d = _lanes(V(ops[1])), _lanes(V(ops[2])), _lanes(V(ops[0]))
            neg = base == 'vnmsubfp128'
            self.setv(ops[0], _pack([_fused(x, y, -z if neg else z, neg) for x, y, z in zip(a, b, d)]))
        elif base == 'vmsum3fp128':
            # Host convention, as native_arithmetic::dot3.
            a, b = _lanes(V(ops[1])), _lanes(V(ops[2]))
            s = to_f32(to_f32(to_f32(a[0] * b[0]) + to_f32(a[1] * b[1])) + to_f32(a[2] * b[2]))
            self.setv(ops[0], _pack([s] * 4))
        elif base in ('vrsqrtefp128', 'vrsqrtefp', 'vrefp'):
            def est(x):
                if base == 'vrefp':
                    return math.copysign(math.inf, x) if x == 0 else to_f32(1 / x)
                if x == 0: return math.inf
                if x < 0: return math.nan
                return to_f32(1 / to_f32(math.sqrt(x)))
            self.setv(ops[0], _pack([est(x) for x in _lanes(V(ops[1]))]))
        elif base == 'vcsxwfp128':
            scale = 2 ** int(ops[2], 0)
            self.setv(ops[0], _pack([self.s32(w) / scale for w in _words(V(ops[1]))]))
        elif base == 'vspltisw128':
            self.setv(ops[0], _packw([int(ops[1], 0)] * 4))
        elif base == 'vspltw128':
            self.setv(ops[0], _packw([_words(V(ops[1]))[int(ops[2], 0)]] * 4))
        elif base == 'vperm128':
            src = V(ops[1]) + V(ops[2]); ctl = V(ops[3])
            self.setv(ops[0], bytes(src[c & 31] for c in ctl))
        elif base == 'vmr':
            self.setv(ops[0], V(ops[1]))
        elif base in ('vor128', 'vandc128', 'vand128', 'vxor128'):
            a, b = _words(V(ops[1])), _words(V(ops[2]))
            op = {'vor128': lambda x, y: x | y, 'vandc128': lambda x, y: x & ~y,
                  'vand128': lambda x, y: x & y, 'vxor128': lambda x, y: x ^ y}[base]
            self.setv(ops[0], _packw([op(x, y) for x, y in zip(a, b)]))
        elif base == 'vsel':
            a, b, c = _words(V(ops[1])), _words(V(ops[2])), _words(V(ops[3]))
            self.setv(ops[0], _packw([(x & ~z) | (y & z) for x, y, z in zip(a, b, c)]))
        elif base == 'vslw128':
            a, b = _words(V(ops[1])), _words(V(ops[2]))
            self.setv(ops[0], _packw([x << (y & 31) for x, y in zip(a, b)]))
        elif base in ('vcmpeqfp128', 'vcmpgefp128', 'vcmpgtfp128'):
            a, b = _lanes(V(ops[1])), _lanes(V(ops[2]))
            test = {'vcmpeqfp128': lambda x, y: x == y, 'vcmpgefp128': lambda x, y: x >= y,
                    'vcmpgtfp128': lambda x, y: x > y}[base]
            result = [test(x, y) for x, y in zip(a, b)]
            self.setv(ops[0], _packw([0xffffffff if t else 0 for t in result]))
            if record:
                self.cr[6] = (LT if all(result) else 0) | (EQ if not any(result) else 0)
        elif base == 'lvlx128':
            ea = ((0 if ops[1] == 'r0' else self.gpr(ops[1])) + self.gpr(ops[2])) & M32
            n = 16 - (ea & 15)
            self.setv(ops[0], self.read(ea, n) + bytes(16 - n))
        elif base == 'mfocrf':
            image = 0
            for field in range(8): image |= (self.cr[field] & 15) << (4 * (7 - field))
            self.set_gpr(ops[0], image)
        else:
            raise Unsupported(f'{pc:08x} {mn} {ops}')
