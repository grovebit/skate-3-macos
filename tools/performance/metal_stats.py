"""Per-run CPU work and presentation from macOS's always-on Metal statistics.

Usage: python tools/performance/metal_stats.py OUTDIR

For a bench.sh output folder: collects the statistics for the runs in
OUTDIR/index.txt (metalperftrace, no profiler attached) and prints per run:

- the process's CPU seconds and instructions over its whole life, loading
  included;
- inside the 60 s sample window, located by each log's "warmup complete" line:
  frames presented, vblanks missed (on-glass time over the vblank period, less
  the frames shown), seconds containing a missed vblank, GPU time per frame,
  the wait for a drawable, and the mean end-to-end time from a frame's first
  command buffer to its presentation completing.

Instructions are the steadiest measure of CPU work: CPU seconds also move
with clocks and contention. Battery and AC power differ; compare runs from
one session only.
"""
import json
import re
import statistics
import subprocess
import sys
from datetime import datetime
from pathlib import Path


def timestamp(text: str) -> float:
    return datetime.fromisoformat(text.replace("Z", "+00:00")).timestamp()


def main():
    out = Path(sys.argv[1])
    runs = [line.split() for line in (out / "index.txt").read_text().splitlines() if line.strip()]
    stats = out / "metal-stats"
    if not any(stats.glob("*.atrc")):
        subprocess.run(["metalperftrace", "collect", "--start", runs[0][2], "--end", runs[-1][3],
                        str(stats)], check=True, stdout=subprocess.DEVNULL)
    trace = next(stats.glob("*.atrc"))
    predicate = " OR ".join(f"pid == {run[1]}" for run in runs)
    data = json.loads(subprocess.run(
        ["metalperftrace", "overview", "--json", "--json-include-timeline", "--predicate", predicate,
         str(trace)], capture_output=True, text=True, check=True).stdout)
    processes = {p["PID"]: p for p in data}
    print(f"{'run':10s} {'cpu_s':>6s} {'instr_G':>7s} {'frames':>6s} {'missed':>6s} "
          f"{'miss_s':>6s} {'gpu_ms':>6s} {'gpu_max':>7s} {'wait_ms':>7s} {'e2e_ms':>6s} {'cb/frame':>8s}")
    for label, pid, *_ in runs:
        process = processes.get(int(pid))
        if not process or not process["Layers"]:
            print(f"{label:10s} no statistics")
            continue
        usage = process["Resource Usage"]
        cpu = usage["CPU User Time (Seconds)"] + usage["CPU Sys Time (Seconds)"]
        log = re.sub(r"\x1b\[[0-9;]*m", "", (out / f"{label}.log").read_text())
        start = timestamp(re.search(r"^(\S+Z) .*SKATE_PERF warmup complete", log, re.M).group(1))
        seconds = [s for s in process["Layers"][0]["Stats Timeline"]
                   if start <= timestamp(s["Start Date"]) < start + 60
                   and s["Presented Frame Stats"]["Frame Count"]]
        glass = [s["Frame-On-Glass Interval Stats"] for s in seconds]
        vblank = statistics.median(g["Min (ms)"] for g in glass if g["Count"])
        missed = [max(round(g["Total (ms)"] / vblank) - g["Count"], 0) for g in glass]
        presented = [s["Presented Frame Stats"] for s in seconds]
        frames = sum(p["Frame Count"] for p in presented)
        gpu = sum(p["On-GPU Walltime Stats"]["Total (ms)"] for p in presented) / frames
        gpu_max = max(p["On-GPU Walltime Stats"]["Max (ms)"] for p in presented)
        wait = sum(p["Next Drawable Wait Walltime Stats"]["Total (ms)"] for p in presented) / frames
        e2e = sum(p["End-to-end Walltime Stats (Total)"]["Total (ms)"] for p in presented) / frames
        buffers = sum(p["Command Buffer Count"] for p in presented) / frames
        print(f"{label:10s} {cpu:6.1f} {usage['Total Instruction Count'] / 1e9:7.1f} {frames:6d} "
              f"{sum(missed):6d} {sum(1 for m in missed if m):6d} {gpu:6.2f} {gpu_max:7.2f} "
              f"{wait:7.2f} {e2e:6.2f} {buffers:8.1f}")


if __name__ == "__main__":
    main()
