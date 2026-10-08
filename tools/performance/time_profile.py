"""Summarise an xctrace Time Profiler capture of the game.

Usage: python tools/performance/time_profile.py CAPTURE.trace [--thread NAME]
           [--top N] [--inclusive REGEX] [--callees REGEX]

Exports the time-profile table beside the capture (CAPTURE.samples.xml, reused
on later runs), demangles Rust symbols and prints, per thread, the heaviest
leaf (self) and inclusive functions. --callees REGEX instead splits the time
below the outermost frame matching REGEX by its direct callee.

Samples are CPU time while running; waits do not appear. Inclusive rows nest
and overlap, so they do not sum.
"""
import argparse
import collections
import re
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path


class _Demangler:
    """Rust v0 symbols (RFC 2603) as `crate::module::item`, `<T as Trait>::item`
    and `item<...>`, without disambiguators or crate hashes."""

    BASIC = dict(a="i8", b="bool", c="char", d="f64", e="str", f="f32", h="u8",
                 i="isize", j="usize", l="i32", m="u32", n="i128", o="u128",
                 s="i16", t="u16", u="()", v="...", x="i64", y="u64", z="!", p="_")
    DIGITS = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"

    def __init__(self, text):
        self.s, self.i, self.depth = text, 0, 0

    def take(self, c):
        if self.s[self.i:self.i + 1] == c:
            self.i += 1
            return True
        return False

    def base62(self):
        if self.take("_"):
            return 0
        value = 0
        while (c := self.s[self.i]) != "_":
            value = value * 62 + self.DIGITS.index(c)
            self.i += 1
        self.i += 1
        return value + 1

    def backref(self, parse):
        target = self.base62()  # an offset into the symbol after "_R"
        saved, self.i, self.depth = self.i, target, self.depth + 1
        if self.depth > 40:
            raise ValueError("backref depth")
        out = parse()
        self.i, self.depth = saved, self.depth - 1
        return out

    def ident(self):
        if self.take("s"):
            self.base62()
        self.take("u")
        digits = re.match(r"\d+", self.s[self.i:]).group()
        self.i += len(digits)
        self.take("_")
        name = self.s[self.i:self.i + int(digits)]
        self.i += int(digits)
        return name

    def path(self):
        c = self.s[self.i]
        self.i += 1
        if c == "C":
            return self.ident()
        if c == "M":
            self.take("s") and self.base62()
            self.path()
            return f"<{self.type()}>"
        if c == "X":
            self.take("s") and self.base62()
            self.path()
            ty = self.type()
            return f"<{ty} as {self.path()}>"
        if c == "Y":
            ty = self.type()
            return f"<{ty} as {self.path()}>"
        if c == "N":
            ns = self.s[self.i]
            self.i += 1
            parent, name = self.path(), self.ident()
            if ns == "C":
                return f"{parent}::{{closure}}"
            if ns == "S":
                return f"{parent}::{{shim}}"
            return f"{parent}::{name}" if name else parent
        if c == "I":
            base, args = self.path(), []
            while not self.take("E"):
                args.append(self.generic_arg())
            return f"{base}<{', '.join(a for a in args if a)}>"
        if c == "B":
            return self.backref(self.path)
        raise ValueError(f"path {c}")

    def generic_arg(self):
        if self.take("L"):
            self.base62()
            return ""
        return self.const() if self.take("K") else self.type()

    def type(self):
        c = self.s[self.i]
        if c in self.BASIC:
            self.i += 1
            return self.BASIC[c]
        self.i += 1
        if c == "A":
            ty = self.type()
            return f"[{ty}; {self.const()}]"
        if c == "S":
            return f"[{self.type()}]"
        if c in "RQ":
            self.take("L") and self.base62()
            return ("&" if c == "R" else "&mut ") + self.type()
        if c in "PO":
            return ("*const " if c == "P" else "*mut ") + self.type()
        if c == "F":
            self.take("G") and self.base62()
            self.take("U")
            if self.take("K") and not self.take("C"):
                self.ident()
            args = []
            while not self.take("E"):
                args.append(self.type())
            return f"fn({', '.join(args)}) -> {self.type()}"
        if c == "D":
            self.take("G") and self.base62()
            traits = []
            while not self.take("E"):
                traits.append(self.path())
                while self.take("p"):
                    self.ident()
                    self.type()
            self.take("L") and self.base62()
            return "dyn " + " + ".join(traits)
        if c == "T":
            items = []
            while not self.take("E"):
                items.append(self.type())
            return f"({', '.join(items)})"
        if c == "B":
            return self.backref(self.type)
        self.i -= 1
        return self.path()

    def const(self):
        if self.take("p"):
            return "_"
        if self.take("B"):
            return self.backref(self.const)
        self.type()
        self.take("n")
        end = self.s.index("_", self.i)
        digits, self.i = self.s[self.i:end], end + 1
        return str(int(digits, 16)) if digits else "0"


def demangle(symbol: str) -> str:
    """Readable form of a Rust v0 symbol; anything else is returned unchanged."""
    if not symbol.startswith("_R") or symbol[2:3].isdigit():
        return symbol
    try:
        return _Demangler(symbol[2:]).path()
    except (ValueError, IndexError, AttributeError):
        return symbol


def export(trace: Path) -> Path:
    out = trace.with_suffix(".samples.xml")
    if not out.exists():
        subprocess.run(
            ["xcrun", "xctrace", "export", "--input", str(trace), "--xpath",
             '/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]',
             "--output", str(out)],
            check=True, stdout=subprocess.DEVNULL)
    return out


def samples(path: Path):
    """Yields (thread, weight_ns, frames innermost first) per sample.

    xctrace writes each distinct value once with an id and later rows refer to
    it, so values are kept by id: rows are cleared as soon as they are read.
    """
    names, stacks, threads, weights = {}, {}, {}, {}
    for _, el in ET.iterparse(path, events=("end",)):
        tag, key = el.tag, el.get("id")
        if key is not None:
            if tag == "frame":
                names[key] = demangle(el.get("name") or el.get("addr") or "?")
            elif tag == "tagged-backtrace":
                stacks[key] = [names[f.get("ref") or f.get("id")] for f in el if f.tag == "frame"]
            elif tag == "thread":
                # "name (0x1234) (skate3rust, pid: 1)": group threads by name.
                threads[key] = re.sub(r"\s*\(0x[0-9a-f]+\).*$", "", el.get("fmt", "?"))
            elif tag == "weight":
                weights[key] = int(el.text or 0)
        if tag != "row":
            continue
        thread = weight = None
        frames = []
        for child in el:
            ref = child.get("ref") or child.get("id")
            if child.tag == "thread":
                thread = threads.get(ref)
            elif child.tag == "weight":
                weight = weights.get(ref)
            elif child.tag == "tagged-backtrace":
                frames = stacks.get(ref, [])
        if thread is not None and weight is not None:
            yield thread, weight, frames
        el.clear()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("trace", type=Path)
    parser.add_argument("--thread", action="append", default=[],
                        help="only threads whose name contains this (repeatable)")
    parser.add_argument("--top", type=int, default=25)
    parser.add_argument("--inclusive", default="", help="regex; only matching inclusive rows")
    parser.add_argument("--callees", default="", help="regex; split time below its outermost match")
    args = parser.parse_args()
    inclusive_filter = re.compile(args.inclusive) if args.inclusive else None
    callee_filter = re.compile(args.callees) if args.callees else None
    totals = collections.Counter()
    self_time = collections.defaultdict(collections.Counter)
    inclusive = collections.defaultdict(collections.Counter)
    below, below_total = collections.Counter(), 0
    for thread, weight, frames in samples(export(args.trace)):
        if args.thread and not any(t in thread for t in args.thread):
            continue
        totals[thread] += weight
        if frames:
            self_time[thread][frames[0]] += weight
        for frame in set(frames):
            inclusive[thread][frame] += weight
        if callee_filter:
            hits = [i for i, f in enumerate(frames) if callee_filter.search(f)]
            if hits:
                below_total += weight
                below[frames[hits[-1] - 1] if hits[-1] else "(self)"] += weight
    if callee_filter:
        print(f"below /{args.callees}/: {below_total / 1e6:.1f} ms")
        for frame, weight in below.most_common(args.top):
            print(f"{weight / 1e6:9.1f} ms {100 * weight / max(below_total, 1):5.1f}%  {frame[:200]}")
        return
    grand = sum(totals.values())
    print(f"sampled CPU {grand / 1e6:.1f} ms")
    for thread, total in totals.most_common():
        if total < grand * 0.01:
            continue
        print(f"\n=== {thread}: {total / 1e6:.1f} ms ({100 * total / grand:.1f}%) ===\n-- self --")
        for frame, weight in self_time[thread].most_common(args.top):
            print(f"{weight / 1e6:9.1f} ms {100 * weight / total:5.1f}%  {frame[:200]}")
        print("-- inclusive --")
        rows = [(f, w) for f, w in inclusive[thread].most_common()
                if not inclusive_filter or inclusive_filter.search(f)]
        for frame, weight in rows[:args.top]:
            print(f"{weight / 1e6:9.1f} ms {100 * weight / total:5.1f}%  {frame[:200]}")


if __name__ == "__main__":
    main()
