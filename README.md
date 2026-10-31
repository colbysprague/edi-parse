# edi-parse

## Usage

    cargo build --release

The binary is `./target/release/edi-parse`. The examples below call it
`edi-parse`.

    edi-parse parse <PATH> [--pretty] [-o, --out <DIR>]
    edi-parse validate <FILE>
    edi-parse summary <DIR> [--sequential] [--top <N>] [-o, --out <DIR>]

Run `edi-parse --help` or `edi-parse <command> --help` for details.

### parse

Parse a file or a directory into JSON.

    edi-parse parse claim.edi --pretty
    edi-parse parse claim.edi --out parsed/
    edi-parse parse claims/ --out parsed/ --pretty

| Option         | Description                                                        |
| -------------- | ------------------------------------------------------------------ |
| `--pretty`     | Pretty-print the JSON.                                             |
| `-o`, `--out`  | Write `.json` files to this directory. Required for a directory.   |

- A file with no `--out` prints its JSON to stdout.
- A directory is searched recursively for `.edi` and `.x12` files, in parallel,
  and the output mirrors its layout.
- Each input is written to `<out>/<path relative to the input dir>/<name>.json`,
  so `claims/a/b.edi` becomes `parsed/a/b.json`.
- A file that fails to parse is reported on stderr and gets no output file. The
  run continues with the remaining files.
- A directory run also writes `<out>/summary.json`, with totals across every file
  found, including subdirectories and failed files.
- A directory run also writes `<out>/validation_errors.json`. See
  [Validation errors](#validation-errors).
- Exits with 1 if any file fails.

### validate

Parse one file and check it for validation errors.

    edi-parse validate claim.edi

Prints `ok`, or a list of validation errors. Exits with 1 if the file has
errors or can't be parsed.

### summary

Process every `.edi` and `.x12` file directly inside a directory. Subdirectories
are not searched.

    edi-parse summary claims/
    edi-parse summary claims/ --top 10 --out parsed/

| Option         | Description                                                 |
| -------------- | ----------------------------------------------------------- |
| `--sequential` | Process files one at a time instead of in parallel.         |
| `--top <N>`    | How many top procedure codes to report. Defaults to 5.      |
| `-o`, `--out`  | Write output files to this directory instead of stdout.     |

Writes summary JSON to `<out>/summary.json`, creating the directory if needed.
With no `--out`, the JSON goes to stdout. A readable report always goes to
stderr. Files that fail are counted in the summary, and the command still exits
with 0.

With `--out`, it also writes `<out>/validation_errors.json`. With no `--out`,
you get the validation error counts only.

### Validation errors

`summary.json` has totals:

```json
"validation_errors": 3,
"validation_error_counts": { "charge_mismatch": 2, "no_service_lines": 1 },
"files_with_validation_errors": 2
```

`validation_errors.json` is an array with every error:

```json
[
  {
    "file": "claims/a.edi",
    "claim_id": "C1",
    "type": "charge_mismatch",
    "message": "declared charge 90.00 != service line total 80.00"
  }
]
```

- `file` is the path as found in the input directory, in the same form as
  `failed_files`.
- `claim_id` is left out for errors with no claim ID, such as a remittance total
  that doesn't match.
- The file is always written, and is `[]` for a clean run. Each run replaces it.
- A parallel run writes files in the order they finish, so entry order can change
  between runs. All of one file's errors stay together. Sort by `file` if you
  need a fixed order.
- Failing to write this file stops the run with an error, and leaves it without
  its closing `]`.
