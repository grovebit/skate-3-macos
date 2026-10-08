"""Per-pass GPU time from an xctrace Metal System Trace of the game.

Usage: python tools/performance/metal_trace.py CAPTURE.trace PID [--frame N]

Exports metal-gpu-intervals beside the capture (CAPTURE.gpu.xml) and keeps the
game's top-level intervals. Prints the merged busy time per frame, then each
pass label's mean time per frame on every GPU channel; --frame N instead lists
the N-th frame's intervals in start order.

Channels of one pass overlap on this tile-based GPU, so rows do not sum to
frame time. A pass waiting on an earlier one also shows a long vertex interval
with little fragment work: read such rows from the frame listing.
"""
import argparse
import collections
import re
import statistics
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path


def export(trace: Path) -> Path:
    out = trace.with_suffix(".gpu.xml")
    if not out.exists():
        subprocess.run(
            ["xcrun", "xctrace", "export", "--input", str(trace), "--xpath",
             '/trace-toc/run[@number="1"]/data/table[@schema="metal-gpu-intervals"]',
             "--output", str(out)],
            check=True, stdout=subprocess.DEVNULL)
    return out


def intervals(path: Path, pid: str):
    """Yields (frame, start_ns, duration_ns, channel, label) for the process."""
    values = {}
    for _, el in ET.iterparse(path, events=("end",)):
        if el.get("id") is not None:
            values[el.get("id")] = (el.text, el.get("fmt"))
        if el.tag != "row":
            continue
        cells = [values.get(c.get("ref") or c.get("id"), (None, None)) for c in el]
        el.clear()
        # start, duration, channel, frame, latency, depth, label, state, uuid, color, process
        if len(cells) < 11 or f"({pid})" not in (cells[10][1] or ""):
            continue
        if cells[5][0] != "0" or cells[3][0] is None:
            continue
        label = re.sub(r"\s+\(.*\)\s+0x[0-9a-f]+$", "", cells[6][1] or "").strip()
        yield int(cells[3][0]), int(cells[0][0]), int(cells[1][0]), cells[2][1], label


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("trace", type=Path)
    parser.add_argument("pid")
    parser.add_argument("--frame", type=int, help="list this frame's intervals instead")
    args = parser.parse_args()
    frames = collections.defaultdict(list)
    for frame, start, duration, channel, label in intervals(export(args.trace), args.pid):
        frames[frame].append((start, duration, channel, label))
    if args.frame is not None:
        spans = sorted(frames[sorted(frames)[args.frame]])
        for start, duration, channel, label in spans:
            print(f"{(start - spans[0][0]) / 1e6:7.3f} +{duration / 1e6:6.3f} {channel:9s} {label}")
        return
    busy = []
    per_label = collections.defaultdict(collections.Counter)
    for spans in frames.values():
        total, end = 0, None
        for start, duration, channel, label in sorted(spans):
            per_label[label][channel] += duration
            stop = start + duration
            if end is None or start > end:
                total, end = total + duration, stop
            elif stop > end:
                total, end = total + stop - end, stop
        busy.append(total / 1e6)
    print(f"frames={len(frames)} busy per frame: mean={statistics.mean(busy):.2f} ms "
          f"median={statistics.median(busy):.2f} max={max(busy):.2f}")
    channels = sorted({c for by in per_label.values() for c in by})
    print(" ".join(f"{c[:8]:>8}" for c in channels) + "  ms per frame, pass")
    for label, by in sorted(per_label.items(), key=lambda kv: -max(kv[1].values())):
        if max(by.values()) / 1e6 / len(frames) >= 0.01:
            print(" ".join(f"{by[c] / 1e6 / len(frames):8.3f}" for c in channels) + f"  {label}")


if __name__ == "__main__":
    main()
