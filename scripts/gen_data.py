#!/usr/bin/env python3
"""Generate N synthetic, valid X12 files (837P claims and 835 remittances).

All names, IDs, and NPIs are fake. Files vary in delimiters, line endings,
and the number of claims/lines, so they exercise the parser like real input.

    python3 scripts/gen_data.py --count 1000 --out data/generated/
"""

import argparse
import os
import random

LAST_NAMES = ["DOE", "ROE", "SMITH", "TESTER", "SAMPLE", "EXAMPLE", "PATIENT", "PLACEHOLDER"]
FIRST_NAMES = ["JANE", "JOHN", "ALEX", "SAM", "PAT", "CASEY", "JORDAN", "TAYLOR"]
PROVIDERS = ["EXAMPLE CLINIC", "SAMPLE MEDICAL GROUP", "TEST FAMILY PRACTICE", "FAKE HEALTH CENTER"]
FAKE_NPI = "1234567893"
# (code, typical charge)
PROCEDURES = [
    ("99211", 45.00), ("99212", 75.00), ("99213", 110.00), ("99214", 165.00),
    ("99215", 225.00), ("85025", 35.00), ("80053", 55.00), ("36415", 12.00),
    ("81002", 18.00), ("97110", 95.00), ("93000", 60.00), ("90471", 30.00),
]
STATUS_CODES = ["1", "1", "1", "1", "2", "3", "4", "19", "22"]

# (element, repetition, component, segment)
DELIMITER_SETS = [
    ("*", "^", ":", "~"),
    ("*", "^", ":", "~"),
    ("*", "^", ":", "~"),
    ("|", "{", ">", "^"),
    ("*", "^", ">", "~"),
]
LINE_ENDINGS = ["", "", "\n", "\r\n"]


def isa(d, control, rng):
    e, rep, comp, seg = d
    s = (
        f"ISA{e}00{e}{'':10}{e}00{e}{'':10}{e}ZZ{e}{'SENDERID':15}{e}ZZ{e}{'RECEIVERID':15}"
        f"{e}{rng.randint(24, 26):02d}{rng.randint(1, 12):02d}{rng.randint(1, 28):02d}"
        f"{e}{rng.randint(0, 23):02d}{rng.randint(0, 59):02d}{e}{rep}{e}00501{e}{control:09d}{e}0{e}T{e}{comp}"
    )
    assert len(s) == 105, len(s)
    return s


def money(x):
    return f"{x:.2f}"


def line_charge(rng, base):
    # Charges in whole cents so sums stay exact when printed with 2 decimals.
    return round(base * rng.uniform(0.8, 1.3), 2)


def body_837(rng, st_control):
    segs = [
        ["ST", "837", st_control, "005010X222A1"],
        ["BHT", "0019", "00", f"REF{st_control}", "20261017", "1200", "CH"],
        ["NM1", "41", "2", "FAKE CLEARINGHOUSE", "", "", "", "", "46", "SUBMIT001"],
        ["HL", "1", "", "20", "1"],
        ["NM1", "85", "2", rng.choice(PROVIDERS), "", "", "", "", "XX", FAKE_NPI],
    ]
    hl = 1
    for c in range(rng.randint(1, 20)):
        hl += 1
        segs.append(["HL", str(hl), "1", "22", "0"])
        segs.append(["SBR", "P", "18", "", "", "", "", "", "", "CI"])
        segs.append(["NM1", "IL", "1", rng.choice(LAST_NAMES), rng.choice(FIRST_NAMES),
                     "", "", "", "MI", f"MEMBER{rng.randint(0, 999999):06d}"])
        lines = [(code, line_charge(rng, base)) for code, base in rng.sample(PROCEDURES, rng.randint(1, 6))]
        total = round(sum(ch for _, ch in lines), 2)
        segs.append(["CLM", f"CLAIM{st_control}{c:03d}", money(total), "", "", "11{C}B{C}1", "Y", "A", "Y", "Y"])
        for i, (code, ch) in enumerate(lines, 1):
            segs.append(["LX", str(i)])
            segs.append(["SV1", f"HC{{C}}{code}", money(ch), "UN", "1", "", "", "1"])
    return segs


def body_835(rng, st_control):
    remits = []
    for c in range(rng.randint(1, 20)):
        status = rng.choice(STATUS_CODES)
        lines = []
        for code, base in rng.sample(PROCEDURES, rng.randint(1, 6)):
            ch = line_charge(rng, base)
            paid = 0.0 if status == "4" else round(ch * rng.uniform(0.5, 1.0), 2)
            lines.append((code, ch, paid))
        charge = round(sum(l[1] for l in lines), 2)
        paid = round(sum(l[2] for l in lines), 2)
        remits.append((f"CLAIM{st_control}{c:03d}", status, charge, paid, lines))
    total_paid = round(sum(r[3] for r in remits), 2)

    segs = [
        ["ST", "835", st_control],
        ["BPR", "I", money(total_paid), "C", "ACH", "CCP", "01", "999999999", "DA", "123456",
         "1234567890", "", "01", "999999999", "DA", "654321", "20261017"],
        ["TRN", "1", f"TRACE{st_control}", "1234567890"],
        ["DTM", "405", "20261017"],
        ["N1", "PR", "EXAMPLE HEALTH PLAN"],
        ["N1", "PE", rng.choice(PROVIDERS), "XX", FAKE_NPI],
    ]
    for claim_id, status, charge, paid, lines in remits:
        segs.append(["CLP", claim_id, status, money(charge), money(paid), money(round(charge - paid, 2)),
                     "12", f"PAYER{claim_id}", "11", "1"])
        segs.append(["NM1", "QC", "1", rng.choice(LAST_NAMES), rng.choice(FIRST_NAMES),
                     "", "", "", "MI", f"MEMBER{rng.randint(0, 999999):06d}"])
        for code, ch, p in lines:
            segs.append(["SVC", f"HC{{C}}{code}", money(ch), money(p), "", "1"])
            if ch - p > 0.004:
                segs.append(["CAS", "CO" if status == "4" else "PR", "2", money(round(ch - p, 2))])
    return segs


def generate(rng, control):
    d = rng.choice(DELIMITER_SETS)
    e, _rep, comp, seg = d
    nl = rng.choice(LINE_ENDINGS)
    is_837 = rng.random() < 0.5
    gs_id, version = ("HC", "005010X222A1") if is_837 else ("HP", "005010X221A1")

    n_txn = rng.choice([1, 1, 1, 2])
    transactions = []
    for t in range(n_txn):
        st_control = f"{t + 1:04d}"
        body = body_837(rng, st_control) if is_837 else body_835(rng, st_control)
        body.append(["SE", str(len(body) + 1), st_control])
        transactions.extend(body)

    segments = [["GS", gs_id, "SENDERID", "RECEIVERID", "20261017", "1200", str(control), "X", version]]
    segments += transactions
    segments.append(["GE", str(n_txn), str(control)])
    segments.append(["IEA", "1", f"{control:09d}"])

    out = [isa(d, control, rng) + seg]
    for s in segments:
        out.append(e.join(s).replace("{C}", comp) + seg)
    return nl.join(out) + nl, is_837


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--count", type=int, required=True, help="number of files to generate")
    ap.add_argument("--out", required=True, help="output directory (created if missing)")
    ap.add_argument("--seed", type=int, default=42, help="random seed (default 42)")
    args = ap.parse_args()

    os.makedirs(args.out, exist_ok=True)
    rng = random.Random(args.seed)
    counts = {"837": 0, "835": 0}
    width = max(5, len(str(args.count)))
    for i in range(1, args.count + 1):
        text, is_837 = generate(rng, i)
        kind = "837" if is_837 else "835"
        counts[kind] += 1
        path = os.path.join(args.out, f"gen_{i:0{width}d}_{kind}.edi")
        with open(path, "w", newline="") as f:
            f.write(text)
    print(f"wrote {args.count} files to {args.out} ({counts['837']} x 837P, {counts['835']} x 835)")


if __name__ == "__main__":
    main()
