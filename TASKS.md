# Tasks

This project builds a real (if small) X12 EDI parser in Rust: you'll take raw
healthcare claim (837P) and remittance (835) files and turn them into typed
data, validated business rules, JSON, and summary statistics. Then you'll make
it parallel and benchmark it.

Every task has **two goals**:

1. **A Rust skill** you practice (lifetimes, enums, `Result`, traits, ...).
2. **A piece of the EDI parser** you add (delimiters, segments, envelope checks, ...).

Each task section below explains both, then gives you the contract, a suggested
approach, questions to check your understanding, hints, and a stretch goal.
Nothing here is a solution: the doc comments on each `todo!()` are the
contract, and the test file is the spec.

---

## How to work

- Work **top to bottom**. Each task builds on the ones before it.
- Every stub is a `todo!()` with its contract in the doc comment above it.
  **Read the docs first**, then read the test file. Tests are the most precise
  spec you have.
- `cargo doc --open` renders every doc comment as a browsable site. Useful when
  you want to see all the types in one place.
- A task is done when its test file passes:

      make test-one T=<file>      # e.g. T=tokenizer_tests
      make test                   # everything; expect failures from later tasks

- Before you write code for a task, run its tests once and read the failures.
  Knowing what "red" looks like makes "green" mean more.
- When you finish, tick the box (`[ ]` → `[x]`) and try the review questions
  without looking at your code.

### Recommended reading

| Rust topic | Where to read |
|---|---|
| Ownership, borrowing, slices | *The Rust Book*, ch. 4 |
| Enums and `match` | *The Rust Book*, ch. 6 |
| `Result`, `?`, error types | *The Rust Book*, ch. 9; the `thiserror` crate docs |
| Generics, traits, lifetimes | *The Rust Book*, ch. 10 |
| Closures and iterators | *The Rust Book*, ch. 13; `std::iter::Iterator` docs |
| Concurrency | *The Rust Book*, ch. 16; the `rayon` crate docs |
| Serialization | serde.rs ("Attributes" section) |

---

## EDI primer: what you're parsing

Read this once before Task 1 and come back to it as needed.

### What X12 EDI is

**EDI** (Electronic Data Interchange) is how businesses exchange structured
documents without humans in the loop. **X12** is the US standard family. In US
healthcare, HIPAA *requires* specific X12 transactions (version 5010) for
claims and payments, so every doctor's office, clearinghouse, and insurer
speaks it. It predates XML and JSON, so it's dense, positional, and terse.

The two transactions in this project form the core of medical billing:

| Transaction | Direction | What it says | Version string |
|---|---|---|---|
| **837P** (Professional Claim) | Provider → Payer | "We saw this patient, did these procedures, and charge this much." | `005010X222A1` |
| **835** (Electronic Remittance Advice, "ERA") | Payer → Provider | "Here's what we paid for each claim, and why we didn't pay the rest." | `005010X221A1` |

A real billing system sends an 837, waits, receives an 835, and then
*reconciles*: matches each payment back to its claim by claim ID and posts the
money. Parsing both reliably is the first step of that pipeline.

### Anatomy of a file

An X12 file is a flat list of **segments**. Each segment is an **ID** followed
by **elements**, separated by delimiters:

```text
CLM*CLAIM001*150.00***11:B:1*Y*A*Y*Y~
│   │        │      ││ │       └─ ... more elements
│   │        │      ││ └─ CLM05: a *composite* element, split on ':' into components
│   │        │      │└─ CLM04: empty, but its position still counts
│   │        │      └─ CLM03: empty
│   │        └─ CLM02: total charge
│   └─ CLM01: claim ID
└─ segment ID
                                     └─ '~' ends the segment
```

Elements are referenced by **segment ID + two-digit position**, 1-based:
`CLM02` is the second element after `CLM`. Empty elements (`**`) are kept so
that positions never shift. Getting this right is the whole game.

The four delimiters (`*` element, `^` repetition, `:` component, `~` segment)
are **not fixed**. Each file declares its own in the `ISA` header (see
`data/samples/837p_pipe_delims.edi` for a file that uses `|`).

### The envelope

Segments nest into three levels of "envelope", each with a header and a
matching trailer that carries a count and a control number:

```text
ISA ─ Interchange header       (who sent it to whom, when; declares delimiters)
  GS ─ Functional group        (a batch of one kind: HC = claims, HP = payments)
    ST ─ Transaction set        (one 837 or one 835)
      ... business segments ...
    SE ─ Transaction trailer    (SE01 = segment count ST..SE, SE02 = ST02)
  GE ─ Group trailer            (GE01 = transaction count, GE02 = GS06)
IEA ─ Interchange trailer       (IEA01 = group count, IEA02 = ISA13)
```

The trailers exist so a receiver can tell if a file was truncated or corrupted
in transit. A mismatched count means "reject this, ask for a resend."

### Segments you'll meet

**837P (claims):**

| Segment | Meaning |
|---|---|
| `HL` | Hierarchical level: builds a tree (billing provider → subscriber → claim). You'll mostly ignore it. |
| `NM1*85` | Billing provider (the clinic). `NM109` is its NPI, the national provider ID. |
| `NM1*IL` | Insured / subscriber (the person with the insurance). |
| `NM1*41` / `NM1*40` | Submitter and receiver (clearinghouse/payer). Not claim parties. |
| `CLM` | Claim header: `CLM01` claim ID, `CLM02` total charge. |
| `SV1` | One professional service line: `SV101` is a composite like `HC:99213` (qualifier `HC` = HCPCS/CPT, procedure code `99213` = office visit), `SV102` charge, `SV104` units. |

**835 (remittances):**

| Segment | Meaning |
|---|---|
| `BPR` | Financial info: `BPR02` is the total actually paid in this transaction (the check/EFT amount). |
| `TRN` | Trace number; links this ERA to the actual bank deposit. |
| `CLP` | Claim payment: `CLP01` your claim ID, `CLP02` status (`1` paid as primary, `4` denied, ...), `CLP03` charged, `CLP04` paid, `CLP05` patient responsibility. |
| `NM1*QC` | The patient this payment is for. |
| `SVC` | Payment for one service line: `SVC01` composite procedure, `SVC02` charged, `SVC03` paid. |
| `CAS` | Adjustment: *why* charged ≠ paid. `CO` = contractual (provider eats it), `PR` = patient responsibility (deductible, coinsurance). |

Look at `data/samples/835_crlf_denial.edi`: `CLAIM201` was denied
(`CLP02 = 4`, paid `0`) with `CAS*CO*50`, a "not medically necessary" denial.
That's the kind of signal billing teams search 835s for.

### The pipeline you're building

```text
&str ──tokenizer──▶ Vec<RawSegment>   Tasks 1-2: split text, borrow everything
     ──segment────▶ Vec<Segment>      Task 3:   typed enum, numeric parsing
     ──envelope───▶ (checked)         Task 4:   structural validation, errors
     ──transaction▶ Interchange       Task 5:   group into claims/remits, JSON
     ──validate───▶ Vec<ValidationError>  Task 6: business rules via a trait
     ──stats──────▶ Summary           Task 7:   aggregate with iterators
many files ─rayon─▶ one Summary       Task 8:   parallelism
                    benchmarks        Task 9:   measure it
```

Note the distinction between **parse errors** (`EdiError`: the file is broken,
stop) and **validation errors** (`ValidationError`: the file is well-formed,
but the numbers don't add up, so report every problem). Real EDI systems keep
these separate too: the first gets a rejection acknowledgment (a "999"), the
second goes to a human work queue.

---

## [x] 1. Zero-copy tokenizer

**Rust skill:** lifetimes and string slices.
**EDI feature:** split raw text into segments and elements.

### Why this step exists

EDI files can be huge (a payer's daily 835 batch can be hundreds of MB). A
naive parser allocates a `String` for every element, which means millions of
tiny heap allocations. Instead, every element here is a `&'a str`: a pointer +
length into the *original input*. Nothing is copied. The cost is that the
compiler now has to prove the input outlives every slice, and that proof is
what lifetimes are.

### What you're building

- **Files:** `src/tokenizer.rs`: `tokenize_with`, `RawSegment::element`
- `tokenize_with(input, delims)` splits on the segment terminator, strips
  line breaks, skips blank segments, splits each segment on the element
  separator, and validates the segment ID.
- `RawSegment::element(n)` looks up an element by its **1-based X12 number**,
  returning `None` for 0, out of range, or empty.

### Concepts to understand

- `RawSegment<'a>` says "this struct holds borrows that live for `'a`."
- `element()` returns `&'a str`, **not** `&str` tied to `&self`. Read
  `slices_outlive_the_segment_vec` in the tests: the slices must remain valid
  after the `Vec<RawSegment>` is dropped. That only works because the return
  lifetime is the *input's*, not the segment's.
- Why are empty elements kept as `""` instead of dropped? (Hint: positions.)

### Done when

`make test-one T=tokenizer_tests`

### Hints

- `str::split`, `str::trim_matches`, and `Iterator::filter` cover most of
  this. A `Result` iterator can `.collect::<Result<Vec<_>, _>>()`.
- For `element(n)`, try `n.checked_sub(1)` so that `n == 0` can't underflow.

### Review questions

- If `element` were declared `fn element(&self, n: usize) -> Option<&str>`,
  which test would stop compiling, and why?
- Your `tokenize_with` builds `out` with a `for` loop and `push`. Could the
  whole function be one iterator chain ending in `collect`? What would the
  `?` turn into?
- `elements[1..].to_vec()` copies a `Vec` of pointers. Does that violate
  "zero-copy"? What *is* and *isn't* being copied?

### Stretch

Return `impl Iterator<Item = Result<RawSegment<'a>, EdiError>>` instead of a
`Vec`, so huge files are tokenized lazily. What does that do to the lifetime
bounds? (You'll likely meet `+ 'a` or `+ '_` on the `impl Trait`.)

---

## [x] 2. Delimiter detection

**Rust skill:** ownership & borrowing; byte vs `char` indexing.
**EDI feature:** read the file's own delimiters from the ISA header.

### Why this step exists

You can't split a file until you know its delimiters, and every file declares
its own. The `ISA` segment is the one fixed-width segment in X12, exactly
106 characters, so the delimiters sit at known byte offsets (3, 82, 104, 105;
see the table at the top of `tokenizer.rs`). Every real parser starts here.

### What you're building

- **Files:** `src/tokenizer.rs`: `Delimiters::detect`, `tokenize`
- `detect` trims leading whitespace, checks for `ISA`, checks length, reads the
  four bytes, and rejects duplicate or alphanumeric delimiters.
- `tokenize` is the convenience wrapper: detect, then `tokenize_with`.

### Concepts to understand

- Contrast with Task 1: there, the return value **borrows** the input.
  Here, you borrow the input, inspect it, and return a small **owned** `Copy`
  value. `Delimiters` doesn't need a lifetime. Why not?
- `&str` can't be indexed by integer (`s[3]` doesn't compile) because UTF-8
  characters are variable width. `.as_bytes()[3]` works because the ISA header
  is ASCII by spec.
- Error-case **order** matters: the tests check that a short non-ISA input is
  `MissingIsa`, not `TruncatedIsa`.

### Done when

`make test-one T=delimiter_tests`

### Hints

- Bytes are safe to index here because the ISA header is ASCII; a `char`
  index into a `&str` isn't a thing. `b[3] as char` works for ASCII.
- Check the error cases in the order the docs list them; the tests rely on it.

### Review questions

- `validate_header` returns `Option<Result<Delimiters, EdiError>>`. Could it
  return `Result<(), EdiError>` instead, so `detect` can just use `?`? Which
  reads better?
- What happens if a file has a multi-byte UTF-8 character inside the first 106
  bytes? Would `as_bytes()[pos] as char` give a sensible answer?
- In the ISA header, ISA11 used to be a "standards identifier" (`U`) in older
  X12 versions (4010) and became the repetition separator in 5010. Why does
  the alphanumeric check skip the repetition separator?

### Stretch

Reject a non-ASCII byte in the header instead of silently mis-reading it, and
add a test for a UTF-8 BOM (`\u{feff}`) before `ISA`. (BOMs show up when files
pass through Windows tools.)

---

## [ ] 3. The `Segment` enum

**Rust skill:** enums with data, exhaustive `match`, `Option`/`Result`
combinators, strict input parsing.
**EDI feature:** turn untyped `RawSegment`s into typed segments the rest of
the program can trust.

### Why this step exists

After Task 2, every element is just a `&str`. The rest of the program wants
`clm.total_charge: f64`, not "element 2 of something whose ID was `CLM`,
hopefully a number." This step is the boundary where you **convert stringly
typed data into real types**, once, and fail loudly if the data is bad. After
this, nothing downstream needs to re-check that `CLM02` is a number.

This is also where enums shine: `Segment` is a closed set of variants, each
carrying exactly the fields that segment has, plus an `Unknown` catch-all so
segments you don't model (`HL`, `DTM`, `CAS`, `LX`, ...) aren't lost.

### What you're building

- **Files:** `src/segment.rs`
- `required(raw, index)`: the "element must be present and non-empty" helper.
- `parse_amount(segment, index, value)`: strict decimal parsing (`-`?, digits,
  at most one `.`, at least one digit).
- `parse_count(segment, index, value)`: digits only, into `usize`.
- `Segment::from_raw(raw, delims)`: one `match raw.id { ... }` that builds every
  variant using the helpers above. Look at each struct's field docs to see
  which X12 element it comes from and whether it's required or optional.
- `Segment::id(&self)`: return the segment ID for any variant.

### Concepts to understand

- **Exhaustive matching.** `id()` must have **no `_` arm**. That way, adding a
  variant later becomes a compile error everywhere it matters instead of a
  silent bug. This is one of Rust's best features; you'll see it pay off in
  the stretch goal.
- **Composite elements.** `SV101`/`SVC01` look like `HC:99213`. Split on
  `delims.component` (not a hard-coded `:`; check the pipe-delimited test).
  If there's no separator, the whole thing is the procedure code.
- **Required vs optional.** Some fields are `&'a str` (required → error if
  missing), some are `Option<&'a str>` (fine if missing), and some are
  `Option<f64>`: optional, **but if present must be a valid number**. That
  last case is the interesting one: "missing" and "garbage" are different
  outcomes. See `sv1_bad_units_is_an_error_not_none`.
- **Trimming.** `ISA06`/`ISA08` are space-padded to fixed width. Which `str`
  method trims only the end?
- **Moving vs cloning.** `from_raw` takes `raw` **by value**. For `Unknown`,
  you can move `raw.elements` into the variant rather than cloning it. The
  test `unknown_elements_are_moved_not_copied` checks for this.
- **Why validate characters yourself?** Rust's `str::parse::<f64>` accepts
  `"NaN"`, `"inf"`, and `"1e5"`; `parse::<usize>` accepts `"+3"`. None of those
  are valid X12 numbers. A parser that's "too accepting" is a correctness bug
  in financial data.

### Suggested approach

1. `required` first. It's small and everything else uses it.
2. `parse_amount` and `parse_count`, then run just their tests
   (`cargo test --test segment_tests parse_`).
3. `from_raw`, one variant at a time. Start with the trailers (`SE`, `GE`,
   `IEA`), then `ST`, `CLM`, and so on. Leave `ISA`, `SV1`, and `SVC` for last.
4. `id()` last, and confirm it has no wildcard arm.

### Questions to check your understanding

- What's the type of "an optional element that must be numeric if present,"
  before and after parsing? Write the type signatures out on paper.
- Why does `required` take `&RawSegment<'a>` but return `&'a str`?
- What would go wrong in Task 5 if `procedure_code` kept the `HC:` prefix?

### Done when

`make test-one T=segment_tests`

### Hints

- For optional numeric elements, `Option<&str>` → `Option<Result<f64, _>>`
  → `Result<Option<f64>, _>` is exactly what `Option::transpose` does.
- `"+3".parse::<usize>()` and `"NaN".parse::<f64>()` both succeed, so check
  the characters yourself before calling `parse`.
- `str::split_once` is handy for composites.

### Stretch

Add a variant for `CAS` (claim adjustments: group code plus reason/amount
pairs) and watch the compiler point at every `match` you need to update.
`CAS` is how an 835 explains *why* a claim wasn't paid in full, so it's the
most valuable segment a real ERA parser handles.

---

## [ ] 4. Error handling for invalid files

**Rust skill:** `Result`, the `?` operator, collecting `Result`s, `match`
guards, carrying state through a loop.
**EDI feature:** structural (envelope) validation: reject files that are
truncated, miscounted, or improperly nested.

### Why this step exists

Files arrive broken all the time: an FTP transfer cut off, a vendor that
miscounts segments, a hand-edited test file. The envelope trailers (`SE`,
`GE`, `IEA`) exist so receivers can detect this. In this step you wire three
layers of failure (delimiters, numbers, envelope) into one `Result` type and
let `?` carry errors up without any `match` boilerplate.

### What you're building

- **Files:** `src/segment.rs` (`parse_segments`), `src/transaction.rs`
  (`check_envelope`, `parse_str`)
- `parse_segments(input)`: detect delimiters once, tokenize, convert each
  `RawSegment` with `from_raw`, and stop at the first error.
- `check_envelope(segments)`: walk the typed segments and enforce, in order:
  starts with `ISA`, ends with `IEA`, every `ST` closed by `SE` before the next
  `ST`/`GE`/`IEA`, `SE01` equals the real count, `SE02` equals `ST02`.
- `parse_str(input)`: the whole pipeline, three fallible calls.

`build_interchange` is still `todo!()`. The invalid-file tests fail *before*
they reach it, so you can finish this task without it. (Running the CLI's
`parse` command on a *valid* file will still panic at that `todo!()` until
Task 5.)

### Concepts to understand

- **`?` and `From`.** `?` returns early with the error. Every function in this
  pipeline returns `EdiError`, so no conversion is needed. In Task 8 you'll
  meet the case where it is.
- **Collecting `Result`s.** An iterator of `Result<T, E>` can collect into
  `Result<Vec<T>, E>`, stopping at the first `Err`. You used this in Task 1;
  here it makes `parse_segments` short.
- **State machines in a loop.** `check_envelope` is a tiny state machine: you
  are either *inside* a transaction (and know where it started and its
  control number) or *outside*. `Option<...>` models that state naturally.
- **Off-by-one in SE01.** The count includes **both** `ST` and `SE`. Count
  segments in `data/samples/835_multi_clp.edi` by hand and compare with its
  `SE01` to confirm.

### Exercise before coding

Open each file in `data/invalid/` and predict **which function** rejects it
(`detect`, `from_raw`, or `check_envelope`) and **which `EdiError` variant**
it produces. Then check yourself against `tests/invalid_tests.rs`.

### Done when

`make test-one T=invalid_tests`

### Hints

- Track the open transaction as `Option<(usize, &str)>` (start index and ST02)
  while you walk `segments.iter().enumerate()`.
- A match guard (`Segment::Ge(_) | Segment::Iea(_) if open.is_some() => ...`)
  keeps the "ST without SE" check to one arm.
- `Segment::id()` from Task 3 gives you the `found` string for
  `UnexpectedSegment`.
- `slice::first` and `slice::last` return `Option`s, which pair well with
  `matches!`.

### Questions to check your understanding

- Why does `check_envelope` take `&[Segment]` while `build_interchange` takes
  `Vec<Segment>`? What does each need to do with the data?
- `EdiError` owns its data (`String`, not `&'a str`). Read the comment at the
  top of `src/error.rs`. What would break if errors borrowed from the input?

### Stretch

Also check `GE01`/`GE02` against the transaction count and `GS06`, and `IEA02`
against `ISA13`. Add files to `data/invalid/` for each one;
`every_invalid_file_fails_to_parse` will pick them up.

---

## [ ] 5. Transaction grouping + JSON

**Rust skill:** ownership transfer (`into_iter`, moving values into structs),
mutable state with `Option` and `last_mut`, serde derive and attributes.
**EDI feature:** turn a flat segment list into the business hierarchy
(interchange → transactions → claims/remits → service lines), and export it as
JSON.

### Why this step exists

X12 is flat, but the meaning is hierarchical: an `SV1` belongs to the most
recent `CLM`, which belongs to the most recent subscriber `NM1*IL`, which
belongs to a billing provider `NM1*85`. Rebuilding that tree from a flat stream
is the core job of any EDI parser. Downstream systems (databases, APIs,
dashboards) want the tree, usually as JSON.

### What you're building

- **Files:** `src/transaction.rs`: `build_interchange`, `to_json`, plus
  `#[derive(Serialize)]` / `#[serde(...)]` on the structs and enum.
- `build_interchange(segments)`: consume the `Vec`, read header fields from
  `ISA`, start a `Transaction` at each `ST` (837 → `Claims`, 835 →
  `Remittance`, else error), and route each segment to the right place per the
  rules in the doc comment.
- `to_json(interchange, pretty)`: one call to `serde_json`, once the derives
  are in place.

### Concepts to understand

- **Consuming iteration.** `into_iter()` on a `Vec<Segment>` yields owned
  `Segment`s, so `match`ing them moves the inner struct out and you can move
  its fields straight into your `Claim`/`Remit`. No clones of the `&str`s
  needed (they're `Copy` anyway, but the inner `Vec`s aren't).
- **"Current" state.** You can't look ahead easily, so keep the current
  billing provider, subscriber, and transaction as state. An `SV1` goes to
  `claims.last_mut()`. If there is no last claim, that's an
  `UnexpectedSegment` error, not a panic.
- **Context carries forward.** In 837s, one `NM1*85` billing provider applies
  to every following claim, while each `CLM` gets the most recent `NM1*IL`.
  `Party` derives `Clone`, and that's intentional.
- **835 patient order.** In an 835, `NM1*QC` comes *after* its `CLP`, the
  opposite of the 837 pattern. Attach it to the remit you just opened.
- **Finishing a transaction.** When you hit `SE`, the current transaction is
  done and moves into `transactions`. What's the cleanest way to "take" a
  value out of an `Option` you own? (Look at `Option::take`.)
- **serde attributes.** You need three things: derive `Serialize`; make the
  enum *internally tagged* so each transaction has a `"type"` field; skip
  `None` fields instead of writing `null`. The `to_json` doc comment and
  `json_*` tests describe the exact shape. Look up `tag`, `rename_all`, and
  `skip_serializing_if` on serde.rs.

### Suggested approach

1. Get `interchange_header_fields` passing: just read the `ISA` and return an
   empty `transactions` list.
2. Add 837 grouping and get the `groups_837_*`, `each_claim_*`, and
   `billing_provider_*` tests passing.
3. Add 835 grouping.
4. Error cases (`unsupported_transaction_set`, `service_line_before_any_claim`).
5. serde last.

### Questions to check your understanding

- Run `cargo run -- parse data/samples/835_multi_clp.edi --pretty`. Does the
  JSON look like something you'd want to `INSERT` into a database? What's
  missing (dates, adjustments, payer name)?
- `parsed_data_borrows_from_input` checks pointer addresses. Why does that
  test exist? What change to `Claim` would make it fail?
- Why does `ServiceLine.paid` exist as `Option<f64>` instead of having two
  separate line types?

### Done when

`make test-one T=transaction_tests`

### Hints

- Keep the "current" transaction/claim as state (`Option<...>` plus
  `.last_mut()` on the claims `Vec`) rather than trying to look ahead.
- `#[derive(Serialize)]` works on structs with `&'a str` fields with no extra
  work. The JSON shape the tests want is in the `to_json` doc comment.

### Stretch

Derive `Deserialize` too, and make a JSON round-trip test pass. (Borrowed
`&'a str` fields need `#[serde(borrow)]`. Why? What happens with a JSON string
that contains an escape like `\"`?)

---

## [ ] 6. The `Validate` trait

**Rust skill:** traits: defining behavior, multiple impls, provided (default)
methods, generic functions with trait bounds.
**EDI feature:** business-rule validation: do the dollars add up?

### Why this step exists

A file can be perfectly well-formed and still be wrong: a claim whose header
says $150 but whose lines sum to $140, a remittance whose check amount doesn't
match the claim payments, a status code that doesn't exist. Payers reject
claims for exactly these reasons, and billing teams reconcile 835s by checking
exactly these sums. Unlike parse errors, you want **every** problem at once,
so a human can fix them in one pass.

A trait is the natural fit: "something that can validate itself" applies to a
claim, a remit, a remittance, a transaction, and a whole interchange, and the
bigger ones are built from the smaller ones.

### What you're building

- **Files:** `src/validate.rs`
- `validate_all<T: Validate>(items)`: validate a slice of anything,
  concatenating all errors.
- `impl Validate for` `Claim`, `Remit`, `Remittance`, `Transaction`,
  `Interchange`. The rules, and their order, are in each impl's doc comment.

### Concepts to understand

- **Provided methods.** `is_valid` is implemented once, in the trait.
  Every implementor gets it for free. You don't write it five times.
- **Generics with bounds.** `validate_all<T: Validate>` is monomorphized: the
  compiler makes a separate copy for `&[Claim]`, `&[Remit]`, etc. Compare with
  `dyn Validate` in the stretch goal.
- **Composition.** `Interchange` validates its transactions; `Transaction`
  delegates to its inner `ClaimSet` or `Remittance`; `Remittance` validates
  its remits *and* checks its own total. Lean on `validate_all` and the
  higher-level impls become tiny.
- **Floating point money.** `0.1 + 0.2 != 0.3` in `f64`. That's why
  `AMOUNT_TOLERANCE` exists. (Real systems often use integer cents or a
  decimal type; ask yourself what you'd change to use `i64` cents.)
- **Errors-or-empty.** The return type is `Result<(), Vec<ValidationError>>`,
  but internally it's easier to build up a `Vec` and convert at the end.
  Think about which direction you need to convert and where.

### Suggested approach

1. `validate_all` first.
2. `Claim`, then run the `claim_*` tests.
3. `Remit`, `Remittance`.
4. `Transaction` and `Interchange` (one or two lines each).

### Questions to check your understanding

- Look at `data/samples/835_crlf_denial.edi`. Does the denied claim pass
  validation? Should it? (See `denied_remit_is_valid`.)
- `CLAIM_STATUS_CODES` includes `22` (reversal of a previous payment). How
  could a reversal legitimately produce a *negative* `paid`, and what would
  that mean for the "`paid` ≥ 0" rule in a real system?
- Why does the remit rule skip line checks when there are no service lines,
  but the claim rule reports `NoServiceLines`?

### Done when

`make test-one T=validate_tests`

### Hints

- Write `validate_all` first, then use it to implement `Transaction` and
  `Interchange` in one or two lines each.
- `Result<(), Vec<E>>` → errors-or-empty: `.err().unwrap_or_default()` is handy.

### Stretch

Make rules pluggable: a
`trait Rule<T> { fn check(&self, item: &T) -> Option<ValidationError>; }` and
a `Vec<Box<dyn Rule<Claim>>>`. When do you need `dyn`, and when would generics
work? (Think: can you put two different rule types in the same `Vec` with
generics alone?)

---

## [ ] 7. Stats with iterator chains

**Rust skill:** iterator adapters (`filter_map`, `flat_map`, `flatten`,
`chain`, `map`, `sum`, `fold`), closures, sorting with comparators, borrowing
from `self` in return values.
**EDI feature:** reporting: totals billed and paid, claim/remit counts,
procedure code frequency.

### Why this step exists

The questions a billing manager actually asks are aggregates: how many claims
did we send, how much did we bill, how much did we get paid, which procedures
do we do most? This step answers them for one file, and defines how to
**merge** two summaries, which is what makes Task 8's parallelism possible.

### What you're building

- **Files:** `src/stats.rs`
- `Summary::from_interchange(&Interchange)`: fill every field from one parsed
  file (including running validation to count errors).
- `Summary::failure(&EdiError)`: a summary representing one failed file.
- `Summary::merge(self, other)`: combine two summaries.
- `Summary::top_procedures(&self, n)`: most frequent procedure codes.

### Concepts to understand

- **Iterator chains instead of loops.** Try writing each field of
  `from_interchange` as a single expression with no `mut` counters. The
  starting point is "all claims across all transactions": iterate
  `transactions`, keep only the claim sets, and flatten their claims.
- **`filter_map` with `match`/`if let`.** Turning a `Transaction` enum into
  "the claims, if this is a claims transaction" is a `filter_map` job.
- **`chain`.** `service_lines` and `procedure_counts` cover *both* 837 and 835
  lines. Two iterators, one stream.
- **Counting into a map.** `BTreeMap`'s `entry` API is the idiomatic way to
  increment a count. Why does `Summary` use `BTreeMap` rather than `HashMap`?
  (Read the field's doc comment, then think about Task 8's tests comparing
  sequential and parallel results.)
- **Monoids.** `merge` with `Summary::default()` as its identity, plus
  associativity (`(a⊕b)⊕c == a⊕(b⊕c)`), is the mathematical property that lets
  you merge in any grouping, which is exactly what a parallel reduce does.
- **Borrowed return values.** `top_procedures` returns `Vec<(&str, usize)>`
  borrowed from `self`'s map keys. Elided lifetimes tie them to `&self`. No
  `clone()` needed.

### Questions to check your understanding

- `merge` takes `self` and `other` by value. Why is that a better fit than
  `&self, &other` for something used in a reduce?
- Is `merge` *commutative* as well as associative? Look at what happens to
  `failed_files` and how the doc says to handle it.
- In `from_interchange`, which fields come from 837 data, which from 835, and
  which from both?

### Done when

`make test-one T=stats_tests`

### Hints

- Start with "all claims across all transactions": an iterator over
  `transactions` that `filter_map`s to the claim `Vec`s and `flatten`s them.
- `top_procedures` returns `&str`s borrowed from `self`'s map keys. Sort with
  `sort_by(|a, b| b.1.cmp(&a.1).then(...))`.
- `Summary { field: ..., ..Default::default() }` (struct update syntax) keeps
  `failure` short.

### Stretch

Implement `std::iter::Sum` for `Summary` so that
`summaries.into_iter().sum::<Summary>()` works, and use it in Task 8.

---

## [ ] 8. Parallel processing with rayon

**Rust skill:** data parallelism with rayon, `Send`/`Sync`, ownership across
threads, filesystem I/O, error wrapping with `map_err`.
**EDI feature:** batch processing: summarize a whole directory of files, where
one bad file never takes down the run.

### Why this step exists

Payers and clearinghouses process thousands of files a day. Files are
independent, so this is "embarrassingly parallel": parse each file on any
core, then merge the summaries. Rust's ownership rules are what make this
*safe* with no locks: the compiler proves no two threads touch the same data.

### What you're building

- **Files:** `src/parallel.rs`
- `collect_files(dir)`: list `.edi`/`.x12` files (case-insensitive), sorted.
- `process_file(path)`: read, `parse_str`, build a `Summary`. Wrap errors so
  the path is included.
- `process_dir(dir, parallel)`: summarize every file and merge, sequentially
  or with rayon, producing identical results either way.

### Concepts to understand

- **Why `process_file` returns `Summary`, not `Interchange`.** The file's
  `String` is a local variable; the `Interchange` borrows from it; the
  `String` is dropped at the end of the function. Returning the borrow would
  dangle. **Try it once** to see the compiler error; it's one of the most
  instructive errors in Rust.
- **`iter()` vs `par_iter()`.** The rayon version should look almost the same
  as the sequential one. The key difference: `fold(init, f)` becomes
  `reduce(|| identity, f)`, because each thread needs its *own* starting value.
- **Why this is safe.** `Summary` is `Send` (all its fields are), and each
  thread builds its own. Nothing is shared and nothing is mutated across
  threads. Look up what would happen if `Summary` contained an `Rc`.
- **Failures become data.** A file that fails to parse becomes
  `Summary::failure(&e)` inside the `map`, so the reduce never sees an `Err`.
  Only "can't read the directory" is a real error for `process_dir`.
- **Wrapping errors.** `EdiError::InFile { path, source: Box<EdiError> }` adds
  context. Why does it need a `Box`? (Try removing it: a recursive enum.)
- **Paths and extensions.** `Path::extension` returns `Option<&OsStr>`, not
  `&str`. Think about how to compare case-insensitively.

### Questions to check your understanding

- `std::fs::read_to_string` returns `io::Error`, but you need `EdiError::Io`.
  Look at `read` in `src/main.rs` for one way to convert.
- Why must `collect_files` sort? What test would become flaky without it?
- If `merge` weren't associative, what kind of bug would you see, and would
  it show up every run?

### Done when

`make test-one T=parallel_tests`, and then the CLI works end to end:

    cargo run -- summary data/samples
    cargo run -- validate data/samples/835_multi_clp.edi

### Hints

- `process_file` can't return the `Interchange`: it borrows the `String` you
  read inside the function. That's why it returns an owned `Summary`. Try
  returning the `Interchange` once to see the compiler error.
- Turn per-file errors into `Summary::failure(&e)` inside the `map`, so one
  bad file can't abort the whole run.
- `use rayon::prelude::*;` brings `par_iter` into scope.

### Stretch

Add a `--threads N` flag that builds a `rayon::ThreadPoolBuilder` pool, and
find where more threads stop helping.

---

## [ ] 9. Benchmark vs Python

**Rust skill:** benchmarking with criterion, `black_box`, release vs debug
builds, profiling.
**EDI feature:** performance: is this fast enough for a real daily batch?

### Why this step exists

"Rust is fast" is a claim; this step makes you measure it. You'll compare
your parser against a deliberately naive Python version (`scripts/baseline.py`)
that does roughly the same work, and compare sequential vs parallel Rust. The
real lesson is *where* time goes: parsing parallelizes well, but reading files
and merging results don't (Amdahl's law), so speedup flattens out.

### What you're building

- **Files:** `benches/parse_bench.rs` (already written; extend it),
  `scripts/baseline.py` (read it to see what it does)
- Generate data, run the benchmarks and the timing comparison, and record
  your numbers.

### Concepts to understand

- **`black_box`** stops the optimizer from noticing a result is unused and
  deleting the work you're trying to time.
- **Release vs debug.** Debug builds are often many times slower than release. Never time a debug
  build. `make baseline` builds release first for this reason.
- **Criterion statistics.** Criterion runs many iterations and reports a
  confidence interval and change vs the last run. Learn to read "change:
  -12.3% (p = 0.00 < 0.05)."
- **Amdahl's law.** If 20% of the work is serial (I/O, merge), the maximum
  speedup is 5×, no matter how many cores you have.

### Done when

`make gen-data N=1000 && make bench && make baseline` all run. Write your
numbers (sequential, parallel, Python) in a comment at the top of
`parse_bench.rs`.

### Hints

- `make baseline` builds release first; never time a debug build.
- Try N=100, 1000, and 10000. Where does parallel start winning, and by how much?

### Questions to check your understanding

- At small N, is parallel slower than sequential? Why might that be?
- Is the Python baseline actually doing the same work as your Rust code? Read
  it and list what it skips. Is the comparison fair?

### Stretch

Profile with `cargo flamegraph` (or `samply`) and make the sequential path at
least 20% faster. Pre-sizing `Vec`s and avoiding `to_string()` in
`procedure_counts` are good places to start.

---

## Finally

Delete `#![allow(unused_variables)]` from `src/lib.rs` and make sure
`make fmt lint test` is clean. Clippy will likely have opinions about your
earlier code (for example, `return x;` at the end of a function); treat its
suggestions as a free code review.

### Where to go next

If you want to keep going with EDI after this:

- **CAS adjustments** (Task 3 stretch), then compute per-claim
  `charged - paid - adjustments` and check that it's zero. That's the real
  835 balancing rule.
- **Claim matching:** given a folder of 837s and 835s, match each `CLP01` to
  its `CLM01` and report unpaid or partly paid claims. That's reconciliation,
  the actual business use.
- **Generate a 999 acknowledgment** for a file that fails `check_envelope`.
  That's what a real receiver sends back.
- **Dates:** parse `DTM`/`DTP` segments into real date types (`chrono` or
  `time`).
