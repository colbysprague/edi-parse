#!/usr/bin/env python3
"""Time CLI commands fairly: warm-up runs first, then many timed runs.

A single `time cmd` measurement is noisy. The first launch of a freshly built
binary pays one-off costs (macOS scans new executables, the page cache is cold),
and the machine adds jitter to every run. This script runs each command a few
times untimed, then times it repeatedly and reports min / median / mean +- stdev
of wall time, plus median CPU time (user+sys) and median peak memory (max RSS).
Comparing CPU time with wall time shows how many cores a parallel run actually
kept busy. Peak RSS is the most physical memory the process held at once,
including the interpreter or runtime itself, not just the data.

    python3 scripts/bench.py --runs 20 \\
        --cmd python "python3 scripts/baseline.py data/generated" \\
        --cmd rust   "./target/release/edi-parse summary data/generated"

Commands are split with shlex and run without a shell, so the timing doesn't
include starting a shell. stdout is discarded. A command that fails stops the
script and prints its stderr, so a broken build can't look like a fast one.
"""

import argparse
import os
import shlex
import statistics
import subprocess
import sys
import time

# ru_maxrss is in bytes on macOS but kilobytes on Linux.
RSS_UNIT = 1 if sys.platform == "darwin" else 1024


def run_once(argv):
    """Run argv once. Returns (wall_seconds, cpu_seconds, peak_rss_bytes)."""
    start = time.perf_counter()
    proc = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    # wait4 returns the child's own resource usage, so CPU time is per run.
    _, status, usage = os.wait4(proc.pid, 0)
    wall = time.perf_counter() - start
    stderr = proc.stderr.read().decode(errors="replace")
    proc.stderr.close()
    code = os.waitstatus_to_exitcode(status)
    if code != 0:
        sys.exit(f"command failed (exit {code}): {shlex.join(argv)}\n{stderr}")
    return wall, usage.ru_utime + usage.ru_stime, usage.ru_maxrss * RSS_UNIT


def fmt_ms(seconds):
    return f"{seconds * 1000:8.1f} ms"


def fmt_mb(nbytes):
    return f"{nbytes / (1024 * 1024):8.1f} MB"


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--cmd", nargs=2, action="append", required=True,
                    metavar=("LABEL", "COMMAND"), help="a command to time (repeatable)")
    ap.add_argument("--runs", type=int, default=10, help="timed runs per command (default 10)")
    ap.add_argument("--warmup", type=int, default=2, help="untimed runs per command first (default 2)")
    args = ap.parse_args()

    results = []
    for label, command in args.cmd:
        argv = shlex.split(command)
        for _ in range(args.warmup):
            run_once(argv)
        samples = [run_once(argv) for _ in range(args.runs)]
        walls = [w for w, _, _ in samples]
        cpus = [c for _, c, _ in samples]
        rss = [r for _, _, r in samples]
        results.append((label, walls, cpus, rss))

    fastest = min(statistics.median(walls) for _, walls, _, _ in results)
    width = max(len(label) for label, _, _, _ in results)
    print(f"{args.runs} runs each after {args.warmup} warm-up runs; wall time unless noted\n")
    print(f"{'':{width}}  {'min':>11}  {'median':>11}  {'mean +- stdev':>22}  {'cpu (median)':>12}  {'peak rss':>11}  {'vs fastest':>10}")
    for label, walls, cpus, rss in results:
        median = statistics.median(walls)
        stdev = statistics.stdev(walls) if len(walls) > 1 else 0.0
        mean_sd = f"{fmt_ms(statistics.mean(walls)).strip()} +- {stdev * 1000:.1f}"
        print(f"{label:{width}}  {fmt_ms(min(walls))}  {fmt_ms(median)}  {mean_sd:>22}"
              f"  {fmt_ms(statistics.median(cpus))}  {fmt_mb(statistics.median(rss))}"
              f"  {median / fastest:9.2f}x")


if __name__ == "__main__":
    main()
