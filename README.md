# edi-parse

Parse, validate, and summarize X12 835 and 837 EDI files.

## Build

    cargo build --release

The binary is `./target/release/edi-parse`. The examples below call it
`edi-parse`.

## Commands

    edi-parse parse <PATH> [--pretty] [-o, --out <DIR>]
    edi-parse validate <FILE>
    edi-parse summary <DIR> [--sequential] [--top <N>]

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
- A file that fails to parse is reported on stderr and gets no output file. The
  run continues with the remaining files.
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
    edi-parse summary claims/ --top 10 > summary.json

| Option         | Description                                                 |
| -------------- | ----------------------------------------------------------- |
| `--sequential` | Process files one at a time instead of in parallel.         |
| `--top <N>`    | How many top procedure codes to report. Defaults to 5.      |

Prints summary JSON to stdout and a readable report to stderr. Files that fail
are counted in the summary, and the command still exits with 0.
