//! `edi-parse`: parse, validate, and summarize X12 835 (remittance) and
//! 837P (professional claim) files.
//!
//! The pipeline, one module per stage:
//!
//! ```text
//! &str ──tokenizer──▶ Vec<RawSegment> ──segment──▶ Vec<Segment>
//!      ──transaction──▶ Interchange ──validate──▶ Result<(), Vec<ValidationError>>
//!      ──stats──▶ Summary            (parallel: many files ──rayon──▶ one Summary)
//! ```
//!
//! Every parsed type borrows from the input text (`'a`), so parsing a file
//! never copies element data. See `TASKS.md` for the order to implement things.

// The `todo!()` stubs don't use their parameters yet. Delete this line once
// every task is done, then make sure `make lint` still passes.
#![allow(unused_variables)]

pub mod error;
pub mod parallel;
pub mod segment;
pub mod segment_id;
pub mod stats;
pub mod tokenizer;
pub mod transaction;
pub mod validate;

pub use error::{EdiError, ValidationError};
pub use parallel::{
    collect_files, collect_files_recursive, parse_dir, parse_to_file, process_dir, process_file,
    write_summary, ParseDirReport,
};
pub use segment::{parse_segments, Segment};
pub use stats::Summary;
pub use tokenizer::{tokenize, tokenize_with, Delimiters, RawSegment};
pub use transaction::{
    build_interchange, check_envelope, parse_str, to_json, Claim, ClaimSet, Interchange, Party,
    Remit, Remittance, ServiceLine, Transaction,
};
pub use validate::{validate_all, Validate};
