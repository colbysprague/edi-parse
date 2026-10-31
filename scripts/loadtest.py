#!/usr/bin/env python3
"""Load test: how big a batch can each implementation get through in a time budget?

Starts at --start files and doubles the batch until every implementation takes
longer than --budget seconds (or --max files is reached). Each size reports
throughput (files/s, MB/s) and peak memory, so you can see whether throughput
holds steady as the batch grows, and where each implementation runs out of time.

    python3 scripts/loadtest.py --bin ./target/release/edi-parse --budget 2

Each batch is the directory data/load/batch, grown (or trimmed) to exactly n
files, so the largest batch is the only data on disk. Files are seeded by their
index, so file i is the same no matter when it was generated. They are real
files, not hard links: on APFS, reading through hard links made the parallel
run about 1.8x slower. The files are read from the OS page cache after the
warm-up run, so this measures parsing throughput, not disk speed.
"""

import argparse
import os
import random
import statistics
import sys

from bench import fmt_mb, run_once
from gen_data import generate

LOAD_DIR = os.path.join("data", "load")
BATCH_DIR = os.path.join(LOAD_DIR, "batch")


def file_name(i):
    # Fixed width so names stay stable as the batch grows.
    return f"load_{i:08d}.edi"


def batch_dir(n):
    """Grow or trim the batch directory to exactly n files. Returns (path, total bytes)."""
    os.makedirs(BATCH_DIR, exist_ok=True)
    have = len(os.listdir(BATCH_DIR))
    for i in range(n + 1, have + 1):
        os.remove(os.path.join(BATCH_DIR, file_name(i)))
    if have < n:
        print(f"  generating files {have + 1}..{n}", file=sys.stderr, flush=True)
        for i in range(have + 1, n + 1):
            # Seed per file so a file's content doesn't depend on when it was generated.
            text, _ = generate(random.Random(i), i)
            with open(os.path.join(BATCH_DIR, file_name(i)), "w", newline="") as f:
                f.write(text)
    total = sum(os.path.getsize(os.path.join(BATCH_DIR, file_name(i))) for i in range(1, n + 1))
    return BATCH_DIR, total


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--bin", required=True, help="path to the release edi-parse binary")
    ap.add_argument("--python", default=sys.executable, help="python for the baseline")
    ap.add_argument("--budget", type=float, default=2.0, help="seconds per batch (default 2)")
    ap.add_argument("--start", type=int, default=1000, help="first batch size (default 1000)")
    ap.add_argument("--max", type=int, default=256_000, help="largest batch size (default 256000)")
    ap.add_argument("--runs", type=int, default=3, help="timed runs per size, median kept (default 3)")
    args = ap.parse_args()

    impls = {
        "python": lambda d: [args.python, "scripts/baseline.py", d],
        "rust-seq": lambda d: [args.bin, "summary", "--sequential", d],
        "rust-par": lambda d: [args.bin, "summary", d],
    }
    active = list(impls)
    best = {}  # label -> (largest n within budget, files/s at that n)
    width = max(len(k) for k in impls)

    print(f"budget {args.budget:g}s per batch, median of {args.runs} runs after 1 warm-up\n")
    print(f"{'files':>8}  {'size':>11}  {'':{width}}  {'wall':>9}  {'files/s':>9}  {'MB/s':>7}  {'peak rss':>11}")
    n = args.start
    while active and n <= args.max:
        path, total = batch_dir(n)
        for label in list(active):
            argv = impls[label](path)
            run_once(argv)  # warm-up: page cache, first-launch costs
            samples = [run_once(argv) for _ in range(args.runs)]
            wall = statistics.median(w for w, _, _ in samples)
            rss = statistics.median(r for _, _, r in samples)
            over = wall > args.budget
            print(f"{n:8d}  {fmt_mb(total)}  {label:{width}}  {wall:8.2f}s  {n / wall:9.0f}"
                  f"  {total / wall / 1e6:7.1f}  {fmt_mb(rss)}{'  over budget' if over else ''}",
                  flush=True)
            if over:
                active.remove(label)
            else:
                best[label] = (n, n / wall)
        print()
        n *= 2

    print(f"largest batch within {args.budget:g}s:")
    for label in impls:
        if label in best:
            size, rate = best[label]
            capped = "  (hit --max, could go higher)" if label in active else ""
            print(f"  {label:{width}}  {size:8d} files  ({rate:,.0f} files/s){capped}")
        else:
            print(f"  {label:{width}}  none (even {args.start} files took over {args.budget:g}s)")


if __name__ == "__main__":
    main()
