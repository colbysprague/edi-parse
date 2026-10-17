# Tasks

Work top to bottom; each task builds on the ones before it. Every stub is a
`todo!()` with its contract in the doc comment above it, so read the docs first.
A task is done when its test file passes.

Run `make test-one T=<file>` (for example `T=tokenizer_tests`) to run one file.
`make test` runs everything; expect failures from later tasks until you get there.

---

## [ ] 1. Zero-copy tokenizer

- **Files:** `src/tokenizer.rs` (`tokenize_with`, `RawSegment::element`)
- **Concept:** lifetimes. `RawSegment<'a>` holds `&'a str` slices of the input,
  so no element is copied. Note that `element()` returns `&'a str`, not a `&str`
  tied to `&self`.
- **Done when:** `make test-one T=tokenizer_tests`
- **Hints:**
  - `str::split`, `str::trim_matches`, and `Iterator::filter` cover most of
    this. A `Result` iterator can `.collect::<Result<Vec<_>, _>>()`.
  - For `element(n)`, try `n.checked_sub(1)` so that `n == 0` can't underflow.
- **Stretch:** return `impl Iterator<Item = Result<RawSegment<'a>, EdiError>>`
  instead of a `Vec`, so huge files are tokenized lazily. What does that do to
  the lifetime bounds?

## [ ] 2. Delimiter detection

- **Files:** `src/tokenizer.rs` (`Delimiters::detect`, `tokenize`)
- **Concept:** ownership & borrowing. You borrow the input (`&str` →
  `.as_bytes()`), index into it, and return a small owned `Copy` value. Compare
  that with Task 1, where the return value borrows the input.
- **Done when:** `make test-one T=delimiter_tests`
- **Hints:**
  - Bytes are safe to index here because the ISA header is ASCII; a `char`
    index into a `&str` isn't a thing. `b[3] as char` works for ASCII.
  - Check the error cases in the order the docs list them; the tests rely on it.
- **Stretch:** reject a non-ASCII byte in the header instead of silently
  mis-reading it, and add a test for a UTF-8 BOM (`\u{feff}`) before `ISA`.

## [ ] 3. The `Segment` enum

- **Files:** `src/segment.rs` (`Segment::from_raw`, `Segment::id`, `required`,
  `parse_amount`, `parse_count`)
- **Concept:** enums + exhaustive `match`. One `match raw.id { ... }` builds
  every variant, and `id()` matches every variant with no `_` arm.
- **Done when:** `make test-one T=segment_tests`
- **Hints:**
  - For optional numeric elements, `Option<&str>` → `Option<Result<f64, _>>`
    → `Result<Option<f64>, _>` is exactly what `Option::transpose` does.
  - `"+3".parse::<usize>()` and `"NaN".parse::<f64>()` both succeed, so check
    the characters yourself before calling `parse`.
- **Stretch:** add a variant for `CAS` (claim adjustments: group code plus
  reason/amount pairs) and watch the compiler point at every `match` you need
  to update.

## [ ] 4. Error handling for invalid files

- **Files:** `src/segment.rs` (`parse_segments`), `src/transaction.rs`
  (`check_envelope`, `parse_str`)
- **Concept:** `Result` + `?`. Errors from three layers (delimiters, numbers,
  envelope) flow up through one return type. `parse_str` should be three `?`s.
  `build_interchange` is still `todo!()`; the invalid-file tests fail before
  they reach it.
- **Done when:** `make test-one T=invalid_tests`
- **Hints:**
  - Track the open transaction as `Option<(usize, &str)>` (start index and ST02)
    while you walk `segments.iter().enumerate()`.
  - A match guard (`Segment::Ge(_) | Segment::Iea(_) if open.is_some() => ...`)
    keeps the "ST without SE" check to one arm.
- **Stretch:** also check `GE01`/`GE02` against the transaction count and
  `GS06`, and `IEA02` against `ISA13`. Add files to `data/invalid/` for each one;
  `every_invalid_file_fails_to_parse` will pick them up.

## [ ] 5. Transaction grouping + JSON

- **Files:** `src/transaction.rs` (`build_interchange`, `to_json`, plus serde
  attributes on the structs/enum)
- **Concept:** ownership (consume the `Vec<Segment>` with `into_iter()` and move
  each piece into the structs you build) and serde derive (`#[derive(Serialize)]`,
  `#[serde(tag = "...")]`, `#[serde(skip_serializing_if = "...")]`).
- **Done when:** `make test-one T=transaction_tests`
- **Hints:**
  - Keep the "current" transaction/claim as state (`Option<...>` plus
    `.last_mut()` on the claims `Vec`) rather than trying to look ahead.
  - `#[derive(Serialize)]` works on structs with `&'a str` fields with no extra
    work. The JSON shape the tests want is in the `to_json` doc comment.
- **Stretch:** derive `Deserialize` too, and make a JSON round-trip test pass.
  (Borrowed `&'a str` fields need `#[serde(borrow)]`. Why?)

## [ ] 6. The `Validate` trait

- **Files:** `src/validate.rs`
- **Concept:** traits: one trait, five impls, a provided default method
  (`is_valid`), and a generic function bounded by the trait (`validate_all`).
- **Done when:** `make test-one T=validate_tests`
- **Hints:**
  - Write `validate_all` first, then use it to implement `Transaction` and
    `Interchange` in one or two lines each.
  - `Result<(), Vec<E>>` → errors-or-empty: `.err().unwrap_or_default()` is handy.
- **Stretch:** make rules pluggable: a `trait Rule<T> { fn check(&self, item: &T) -> Option<ValidationError>; }`
  and a `Vec<Box<dyn Rule<Claim>>>`. When do you need `dyn`, and when would generics work?

## [ ] 7. Stats with iterator chains

- **Files:** `src/stats.rs`
- **Concept:** iterators: `filter_map`, `flat_map`, `chain`, `map`, `sum`,
  `fold`. Try writing each field of `from_interchange` as one expression, without
  `for` loops or `mut` counters.
- **Done when:** `make test-one T=stats_tests`
- **Hints:**
  - Start with "all claims across all transactions": an iterator over
    `transactions` that `filter_map`s to the claim `Vec`s and `flatten`s them.
  - `top_procedures` returns `&str`s borrowed from `self`'s map keys. Sort with
    `sort_by(|a, b| b.1.cmp(&a.1).then(...))`.
- **Stretch:** implement `std::iter::Sum` for `Summary` so that
  `summaries.into_iter().sum::<Summary>()` works, and use it in Task 8.

## [ ] 8. Parallel processing with rayon

- **Files:** `src/parallel.rs`
- **Concept:** concurrency with rayon. `par_iter().map(..).reduce(..)` instead of
  `iter().map(..).fold(..)`. It's safe because `Summary` is `Send` and `merge` is
  associative, with `Summary::default` as its identity.
- **Done when:** `make test-one T=parallel_tests`
- **Hints:**
  - `process_file` can't return the `Interchange`: it borrows the `String` you
    read inside the function. That's why it returns an owned `Summary`. Try
    returning the `Interchange` once to see the compiler error.
  - Turn per-file errors into `Summary::failure(&e)` inside the `map`, so one
    bad file can't abort the whole run.
- **Stretch:** add a `--threads N` flag that builds a
  `rayon::ThreadPoolBuilder` pool, and find where more threads stop helping.

## [ ] 9. Benchmark vs Python

- **Files:** `benches/parse_bench.rs` (already written; extend it), `scripts/baseline.py`
- **Concept:** benchmarking: criterion statistics, `black_box`, release vs
  debug builds, and Amdahl's law (I/O and merging don't parallelize).
- **Done when:** `make gen-data N=1000 && make bench && make baseline` all run.
  Write your numbers (sequential, parallel, Python) in a comment at the top of
  `parse_bench.rs`.
- **Hints:**
  - `make baseline` builds release first; never time a debug build.
  - Try N=100, 1000, and 10000. Where does parallel start winning, and by how much?
- **Stretch:** profile with `cargo flamegraph` (or `samply`) and make the
  sequential path at least 20% faster. Pre-sizing `Vec`s and avoiding
  `to_string()` in `procedure_counts` are good places to start.

---

**Finally:** delete `#![allow(unused_variables)]` from `src/lib.rs` and make
sure `make fmt lint test` is clean.
