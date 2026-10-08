#!/bin/bash
# Alternating SKATE_PERF_REPORT runs of game builds, launched as ./play.sh does
# (repository mods and skating audio). Each run enters fullscreen, warms up for
# 10 s, then samples the 60 s flythrough (diagnostics/performance.rs).
#
#   tools/performance/bench.sh OUTDIR ROUNDS 'LABEL|BINARY|ENV=VALUE ...' ...
#
# A binary may be copied out of target/ with its libbevy_dylib-*.dylib beside
# it, so builds can be compared side by side. PROFILE=cpu attaches the Time
# Profiler for 55 s of the flight (time_profile.py); PROFILE=metal records a
# 5 s Metal System Trace 35 s into the run (metal_trace.py). OUTDIR/index.txt
# lists label, pid, start and end for metal_stats.py. Run from a checkout whose
# data/ holds a converted game. Check `pmset -g batt` first: battery power
# changes GPU clocks.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
inst="$repo/data/$(plutil -extract directory raw -o - "$repo/data/installation.json" 2>/dev/null)" ||
    { echo "No converted game in $repo/data: run ./play.sh in this checkout first" >&2; exit 1; }
out=$1; rounds=$2; shift 2
mkdir -p "$out"
for round in $(seq 1 "$rounds"); do
    for variant in "$@"; do
        IFS='|' read -r label bin extra <<<"$variant"
        echo "round $round: $label ($(pmset -g batt | grep -o '[0-9]*%; [a-z]*'))"
        start=$(date -u +%Y-%m-%dT%H:%M:%SZ)
        env $extra SKATE_REPORT_CHILD=1 SKATE3_MODS="$repo/mods" \
            SKATE3_AUDIO="$repo/.local/skating-audio" \
            SKATE_PERF_REPORT="$out/$label-$round.json" \
            "$bin" --assets "$inst/assets" >"$out/$label-$round.log" 2>&1 &
        pid=$!
        case "${PROFILE:-}" in
            cpu) sleep 12; template='Time Profiler'; limit=55s ;;
            metal) sleep 35; template='Metal System Trace'; limit=5s ;;
            *) template= ;;
        esac
        if [ -n "$template" ]; then
            xcrun xctrace record --template "$template" --attach $pid --time-limit $limit \
                --output "$out/$label-$round.trace" >"$out/$label-$round.xctrace.log" 2>&1
        fi
        # A run takes about 72 s. Bevy 0.18 sometimes deadlocks on exit after
        # the report is written; stop it rather than wait.
        for _ in $(seq 1 100); do kill -0 $pid 2>/dev/null || break; sleep 1; done
        if kill -0 $pid 2>/dev/null; then
            echo "$label-$round $pid shutdown hang" >>"$out/hangs.txt"
            kill -TERM $pid
        fi
        wait $pid 2>/dev/null
        echo "$label-$round $pid $start $(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"$out/index.txt"
        echo "  pid=$pid $(grep -h '^SKATE_PERF fps' "$out/$label-$round.log")"
        sleep 5
    done
done
