//! Error types. These are complete: you construct them, you don't edit them.
//!
//! Note that errors own their data (`String`, not `&'a str`). An error often
//! needs to outlive the input buffer it came from (e.g. when a file fails inside
//! `process_dir` and the error message is kept in the `Summary`), so it can't
//! borrow from it.

use std::path::PathBuf;

use thiserror::Error;

/// Anything that stops a file from being parsed.
#[derive(Debug, Error)]
pub enum EdiError {
    /// Reading a file or directory failed.
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The input (after leading whitespace) does not start with `ISA`.
    #[error("input does not begin with an ISA segment")]
    MissingIsa,

    /// The input starts with `ISA` but is shorter than a full ISA segment.
    #[error("ISA header is truncated: need at least {expected} characters, found {found}")]
    TruncatedIsa { expected: usize, found: usize },

    /// The ISA header declares delimiters that can't work (duplicates,
    /// alphanumeric characters, ...).
    #[error("invalid delimiters in ISA header: {0}")]
    InvalidDelimiters(String),

    /// A segment ID is empty, too long, or not uppercase alphanumeric.
    #[error("invalid segment id {id:?} at segment {position}")]
    InvalidSegmentId { position: usize, id: String },

    /// A required element is absent or empty.
    #[error("{segment}{index:02} is required but missing")]
    MissingElement { segment: String, index: usize },

    /// An element that must be numeric isn't.
    #[error("{segment}{index:02}: expected a number, found {value:?}")]
    InvalidNumber {
        segment: String,
        index: usize,
        value: String,
    },

    /// `SE01` disagrees with the real number of segments from `ST` to `SE`.
    #[error("transaction {control_number}: SE01 declares {declared} segments but found {actual}")]
    SegmentCountMismatch {
        control_number: String,
        declared: usize,
        actual: usize,
    },

    /// A trailer's control number doesn't match its header (`SE02`/`ST02`,
    /// `GE02`/`GS06`, `IEA02`/`ISA13`).
    #[error("{segment} control number {found:?} does not match header {expected:?}")]
    ControlNumberMismatch {
        segment: String,
        expected: String,
        found: String,
    },

    /// The interchange has no closing `IEA` segment.
    #[error("interchange is missing its IEA trailer")]
    MissingIea,

    /// A segment appeared where the envelope/loop structure doesn't allow it.
    #[error("unexpected {found} segment at position {position}: expected {expected}")]
    UnexpectedSegment {
        position: usize,
        expected: String,
        found: String,
    },

    /// `ST01` names a transaction set this tool doesn't handle.
    #[error("unsupported transaction set {0:?} (expected 835 or 837)")]
    UnsupportedTransaction(String),

    /// Wraps another error with the file it came from.
    #[error("{path}: {source}")]
    InFile {
        path: PathBuf,
        #[source]
        source: Box<EdiError>,
    },
}

/// A business-rule problem in a file that parsed successfully.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ValidationError {
    #[error("claim has an empty claim id")]
    EmptyClaimId,

    #[error("claim {claim_id}: has no service lines")]
    NoServiceLines { claim_id: String },

    #[error("claim {claim_id}: {field} is negative ({value:.2})")]
    NegativeAmount {
        claim_id: String,
        field: &'static str,
        value: f64,
    },

    #[error("claim {claim_id}: declared charge {declared:.2} != service line total {computed:.2}")]
    ChargeMismatch {
        claim_id: String,
        declared: f64,
        computed: f64,
    },

    #[error(
        "claim {claim_id}: declared payment {declared:.2} != service line payments {computed:.2}"
    )]
    PaymentMismatch {
        claim_id: String,
        declared: f64,
        computed: f64,
    },

    #[error("claim {claim_id}: unknown claim status code {code:?}")]
    UnknownStatus { claim_id: String, code: String },

    #[error("BPR02 total payment {declared:.2} != sum of CLP04 payments {computed:.2}")]
    RemittanceTotalMismatch { declared: f64, computed: f64 },
}
