"""Decode the VMX128 subset used by the original collision-position producer.

Encoding reference: Xenia ppc_instr.h and ppc_opcode_table_gen.cc:
https://github.com/xenia-project/xenia/tree/master/src/xenia/cpu/ppc
Field extraction is independently expressed here. This is not an emulator or
a full PowerPC disassembler. Decode this subset before ordinary VMX opcodes:
some VMX128 instructions otherwise look like unrelated standard instructions.
Unrecognized encodings return None; callers must not infer their semantics.
"""

_ARITHMETIC = {
    0x14000010: 'vaddfp128', 0x14000050: 'vsubfp128',
    0x14000090: 'vmulfp128', 0x140000D0: 'vmaddfp128',
    0x14000110: 'vmaddcfp128', 0x14000150: 'vnmsubfp128',
    0x14000190: 'vmsum3fp128', 0x140001D0: 'vmsum4fp128',
    0x14000210: 'vand128', 0x14000250: 'vandc128',
    0x14000290: 'vnor128', 0x140002D0: 'vor128',
    0x14000310: 'vxor128', 0x14000350: 'vsel128',
    0x18000280: 'vmaxfp128', 0x180002C0: 'vminfp128',
    0x180000D0: 'vslw128',
}
_MEMORY = {
    0x100000C3: 'lvx128', 0x100001C3: 'stvx128',
    0x10000403: 'lvlx128', 0x10000443: 'lvrx128',
}
_IMMEDIATE = {
    0x18000230: 'vcfpsxws128', 0x18000270: 'vcfpuxws128',
    0x180002B0: 'vcsxwfp128', 0x180002F0: 'vcuxwfp128',
    0x18000730: 'vspltw128',
}
_UNARY = {0x18000630: 'vrefp128', 0x18000670: 'vrsqrtefp128'}
_COMPARE = {
    0x18000000: 'vcmpeqfp128', 0x18000080: 'vcmpgefp128',
    0x18000100: 'vcmpgtfp128', 0x18000200: 'vcmpequw128',
}


def decode(word):
    """Return (mnemonic, operands) for one numeric big-endian u32 instruction.

    Three-register arithmetic uses encoded D,A,B order. For destructive forms,
    D is also an input; this display does not expand their arithmetic formula.
    """
    if not 0 <= word <= 0xFFFFFFFF:
        raise ValueError('Instruction must be an unsigned 32-bit word')
    d = ((word >> 21) & 31) | (((word >> 2) & 3) << 5)
    a = ((word >> 16) & 31) | (((word >> 5) & 1) << 5) | (((word >> 10) & 1) << 6)
    b = ((word >> 11) & 31) | ((word & 3) << 5)
    immediate = (word >> 16) & 31
    if name := _ARITHMETIC.get(word & 0xFC0003D0):
        return name, f'v{d}, v{a}, v{b}'
    if name := _MEMORY.get(word & 0xFC0007F3):
        return name, f'v{d}, r{immediate}, r{(word >> 11) & 31}'
    if name := _IMMEDIATE.get(word & 0xFC0007F0):
        return name, f'v{d}, v{b}, {immediate}'
    if name := _UNARY.get(word & 0xFC0007F0):
        return name, f'v{d}, v{b}'
    if word & 0xFC0007F0 == 0x18000770:
        signed = immediate if immediate < 16 else immediate - 32
        return 'vspltisw128', f'v{d}, {signed}'
    if name := _COMPARE.get(word & 0xFC000390):
        return name + ('.' if word & 0x40 else ''), f'v{d}, v{a}, v{b}'
    if word & 0xFC000210 == 0x14000000:
        return 'vperm128', f'v{d}, v{a}, v{b}, v{(word >> 6) & 7}'
    if word & 0xFC000630 == 0x18000210:
        perm = immediate | (((word >> 6) & 7) << 5)
        return 'vpermwi128', f'v{d}, v{b}, {perm:#04x}'
    return None
