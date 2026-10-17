# edi-parse

A Rust learning project: a CLI that reads a folder of X12 EDI files (835
remittances, 837P claims), parses them zero-copy, validates them, and prints
JSON plus summary stats, processing files in parallel with rayon.

The CLI, types, tests, and sample data are done. The parsing logic is `todo!()`
stubs for you to write. **Start with [TASKS.md](TASKS.md).**

    make help                      # list targets
    make test-one T=tokenizer_tests
    cargo run -- parse data/samples/835_multi_clp.edi --pretty
    cargo run -- summary data/samples

All sample data is synthetic (fake names, IDs, and the test NPI `1234567893`).
