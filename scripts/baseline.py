#!/usr/bin/env python3
"""Naive, single-threaded Python version of `edi-parse summary`, for timing.

Does roughly the same work as the Rust pipeline (detect delimiters, split,
check SE counts, group claims/remits, sum totals, count procedures, check the
claim charge rule) so the comparison is fair-ish. It is intentionally written
the obvious way, not optimized.

    python3 scripts/baseline.py data/generated
"""

import json
import os
import sys
from collections import Counter

EXTENSIONS = (".edi", ".x12")


def parse_file(text):
    text = text.lstrip()
    if not text.startswith("ISA"):
        raise ValueError("missing ISA")
    if len(text) < 106:
        raise ValueError("truncated ISA")
    elem, comp, term = text[3], text[104], text[105]

    segments = []
    for raw in text.split(term):
        raw = raw.strip("\r\n")
        if raw:
            parts = raw.split(elem)
            segments.append((parts[0], parts[1:]))
    if not segments or segments[-1][0] != "IEA":
        raise ValueError("missing IEA")

    stats = Counter()
    procedures = Counter()
    billed = paid = 0.0
    errors = 0
    st_index = None
    claim = None  # (declared, [line charges])

    def close_claim():
        nonlocal errors
        if claim is not None and abs(claim[0] - sum(claim[1])) > 0.005:
            errors += 1

    for i, (sid, el) in enumerate(segments):
        if sid == "ST":
            st_index = i
            stats["transactions"] += 1
        elif sid == "SE":
            if int(el[0]) != i - st_index + 1:
                raise ValueError("SE count mismatch")
            close_claim()
            claim = None
        elif sid == "CLM":
            close_claim()
            amount = float(el[1])
            claim = (amount, [])
            billed += amount
            stats["claims"] += 1
        elif sid == "CLP":
            float(el[2])  # validate CLP03
            paid += float(el[3])
            stats["remits"] += 1
        elif sid in ("SV1", "SVC"):
            code = el[0].split(comp)[-1]
            charge = float(el[1])
            procedures[code] += 1
            stats["service_lines"] += 1
            if sid == "SV1" and claim is not None:
                claim[1].append(charge)
    return stats, procedures, billed, paid, errors


def main():
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <dir>")
    directory = sys.argv[1]
    files = sorted(
        os.path.join(directory, f)
        for f in os.listdir(directory)
        if f.lower().endswith(EXTENSIONS) and os.path.isfile(os.path.join(directory, f))
    )

    total = Counter()
    procedures = Counter()
    billed = paid = 0.0
    failed = []
    for path in files:
        try:
            with open(path, newline="") as f:
                s, p, b, pd, errs = parse_file(f.read())
        except (ValueError, IndexError) as e:
            failed.append(f"{path}: {e}")
            continue
        total += s
        total["validation_errors"] += errs
        procedures += p
        billed += b
        paid += pd

    print(json.dumps({
        "files_processed": len(files) - len(failed),
        "files_failed": len(failed),
        "failed_files": failed,
        "transactions": total["transactions"],
        "claims": total["claims"],
        "remits": total["remits"],
        "service_lines": total["service_lines"],
        "total_billed": round(billed, 2),
        "total_paid": round(paid, 2),
        "validation_errors": total["validation_errors"],
        "procedure_counts": dict(sorted(procedures.items())),
    }, indent=2))


if __name__ == "__main__":
    main()
