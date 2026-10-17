//! Task 4 (part): envelope checks (`check_envelope`, `parse_str`).
//! Task 5: group segments into claims/remits, and serialize them to JSON.
//!
//! ```text
//! ISA                       interchange
//!   GS                        functional group
//!     ST                        transaction set (835 or 837)
//!       ...segments...
//!     SE
//!   GE
//! IEA
//! ```
//!
//! For Task 5 you will add `#[derive(Serialize)]` and `#[serde(...)]`
//! attributes to the types below. They are deliberately left without them.

use crate::error::EdiError;
use crate::segment::Segment;

/// A person or organization from an `NM1` segment.
#[derive(Debug, Clone, PartialEq)]
pub struct Party<'a> {
    /// NM103.
    pub last_name: &'a str,
    /// NM104 (absent for organizations).
    pub first_name: Option<&'a str>,
    /// NM109 (NPI, member ID, ...).
    pub id: Option<&'a str>,
}

/// One service line: an `SV1` (837P) or an `SVC` (835).
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceLine<'a> {
    pub procedure_code: &'a str,
    pub charge: f64,
    /// `None` for 837 lines (nothing has been paid yet), `Some` for 835 lines.
    pub paid: Option<f64>,
}

/// An 837P claim: a `CLM` plus its `SV1` lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Claim<'a> {
    /// CLM01.
    pub claim_id: &'a str,
    /// CLM02.
    pub total_charge: f64,
    /// The most recent `NM1*85` seen before this `CLM`.
    pub billing_provider: Option<Party<'a>>,
    /// The most recent `NM1*IL` seen before this `CLM`.
    pub subscriber: Option<Party<'a>>,
    pub service_lines: Vec<ServiceLine<'a>>,
}

/// An 835 claim payment: a `CLP` plus its `SVC` lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Remit<'a> {
    /// CLP01.
    pub claim_id: &'a str,
    /// CLP02.
    pub status_code: &'a str,
    /// CLP03.
    pub total_charge: f64,
    /// CLP04.
    pub paid: f64,
    /// CLP05.
    pub patient_responsibility: Option<f64>,
    /// The `NM1*QC` that follows this `CLP`.
    pub patient: Option<Party<'a>>,
    pub service_lines: Vec<ServiceLine<'a>>,
}

/// The contents of an 837 transaction set.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimSet<'a> {
    /// ST02.
    pub control_number: &'a str,
    pub claims: Vec<Claim<'a>>,
}

/// The contents of an 835 transaction set.
#[derive(Debug, Clone, PartialEq)]
pub struct Remittance<'a> {
    /// ST02.
    pub control_number: &'a str,
    /// BPR02.
    pub total_payment: f64,
    pub remits: Vec<Remit<'a>>,
}

/// One `ST`..`SE` transaction set.
#[derive(Debug, Clone, PartialEq)]
pub enum Transaction<'a> {
    Claims(ClaimSet<'a>),
    Remittance(Remittance<'a>),
}

/// A whole parsed file: one `ISA`..`IEA` interchange.
#[derive(Debug, Clone, PartialEq)]
pub struct Interchange<'a> {
    /// ISA06 (trimmed).
    pub sender_id: &'a str,
    /// ISA08 (trimmed).
    pub receiver_id: &'a str,
    /// ISA13.
    pub control_number: &'a str,
    /// Every transaction set from every functional group, in file order.
    pub transactions: Vec<Transaction<'a>>,
}

/// Check the envelope structure without building anything.
///
/// In order:
/// 1. Empty input, or a first segment that isn't `ISA` → [`EdiError::MissingIsa`].
/// 2. Last segment isn't `IEA` → [`EdiError::MissingIea`].
/// 3. Every `ST` is closed by an `SE` before the next `ST`/`GE`/`IEA`;
///    otherwise [`EdiError::UnexpectedSegment`] naming the segment found.
/// 4. `SE01` equals the count of segments from `ST` to `SE` inclusive →
///    else [`EdiError::SegmentCountMismatch`].
/// 5. `SE02` equals `ST02` → else [`EdiError::ControlNumberMismatch`] with
///    `segment: "SE"`.
pub fn check_envelope(segments: &[Segment<'_>]) -> Result<(), EdiError> {
    todo!("Task 4: envelope checks")
}

/// Group a checked segment list into an [`Interchange`].
///
/// Takes the `Vec` by value: iterate it with `into_iter()` and move each
/// segment's data into the structs you build.
///
/// Rules:
/// - `ST01` of `837` → [`Transaction::Claims`]; `835` → [`Transaction::Remittance`];
///   anything else → [`EdiError::UnsupportedTransaction`].
/// - 837: `NM1*85` sets the current billing provider, `NM1*IL` the current
///   subscriber; each `CLM` starts a claim carrying copies of both; `SV1`
///   adds a line to the current claim.
/// - 835: `BPR02` is the transaction's total payment (0.0 if there is no
///   `BPR`); each `CLP` starts a
///   remit; the `NM1*QC` after it is its patient; `SVC` adds a line.
/// - An `SV1`/`SVC` with no open claim/remit → [`EdiError::UnexpectedSegment`].
/// - All other segments (`HL`, `DTM`, `CAS`, other `NM1`s, ...) are ignored.
pub fn build_interchange<'a>(segments: Vec<Segment<'a>>) -> Result<Interchange<'a>, EdiError> {
    todo!("Task 5: group segments into transactions")
}

/// Parse a whole file: `parse_segments` → `check_envelope` → `build_interchange`.
/// Three fallible steps; this should be three lines and three `?`s.
pub fn parse_str<'a>(input: &'a str) -> Result<Interchange<'a>, EdiError> {
    todo!("Task 4: wire the pipeline together with ?")
}

/// Serialize an interchange to JSON (pretty-printed if `pretty`).
///
/// Expected shape (see `tests/transaction_tests.rs` for the exact checks):
/// - field names exactly as the Rust fields (`claim_id`, `service_lines`, ...);
/// - each transaction has `"type": "claims"` or `"type": "remittance"` next to
///   its own fields (an internally tagged enum);
/// - `Option` fields that are `None` are omitted, not `null`.
pub fn to_json(interchange: &Interchange<'_>, pretty: bool) -> Result<String, serde_json::Error> {
    todo!("Task 5: derive Serialize on the types above, then call serde_json")
}
