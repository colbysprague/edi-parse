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
| `-o`, `--out`  | Write `summary.json` to this directory instead of stdout.   |

Writes summary JSON to `<out>/summary.json`, creating the directory if needed.
With no `--out`, the JSON goes to stdout. A readable report always goes to
stderr. Files that fail are counted in the summary, and the command still exits
with 0.
