#!/usr/bin/env bash
# Runs every edi-parse subcommand on examples/inputs and writes the results to
# examples/output. Run from anywhere:  ./examples/run.sh
set -uo pipefail

cd "$(dirname "$0")/.."
cargo build --release --quiet || exit 1
BIN=./target/release/edi-parse
IN=examples/inputs
OUT=examples/output
rm -rf "$OUT" && mkdir -p "$OUT/parsed"

echo "== summary: whole directory =="
# JSON goes to stdout (saved), the human-readable report to stderr (shown).
"$BIN" summary "$IN" > "$OUT/summary.json"

echo
echo "== parse: one JSON file per input =="
for f in "$IN"/*.edi; do
    name=$(basename "$f" .edi)
    if "$BIN" parse "$f" --pretty > "$OUT/parsed/$name.json" 2> "$OUT/parsed/$name.err"; then
        rm "$OUT/parsed/$name.err"
        echo "  ok    $name.json"
    else
        rm "$OUT/parsed/$name.json"
        echo "  error $name.err: $(cat "$OUT/parsed/$name.err")"
    fi
done

echo
echo "== validate: exit code per file =="
for f in "$IN"/*.edi; do
    if "$BIN" validate "$f" > /dev/null 2>&1; then
        echo "  valid    $(basename "$f")"
    else
        echo "  invalid  $(basename "$f")"
    fi
done

echo
echo "outputs written to $OUT/:"
find "$OUT" -type f | sort | sed 's/^/  /'
