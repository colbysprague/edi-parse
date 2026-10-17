//! Task 3: turn `RawSegment`s into a typed `Segment` enum.
//! Task 4 (part): `parse_segments`, the tokenize → typed-segments pipeline.
//!
//! Element numbers in the docs below are X12 numbers (`CLM02` = `raw.element(2)`).

use crate::error::EdiError;
use crate::tokenizer::{Delimiters, RawSegment};

/// `ISA`: interchange header.
#[derive(Debug, Clone, PartialEq)]
pub struct Isa<'a> {
    /// ISA06, with trailing pad spaces trimmed.
    pub sender_id: &'a str,
    /// ISA08, with trailing pad spaces trimmed.
    pub receiver_id: &'a str,
    /// ISA09, `YYMMDD`.
    pub date: &'a str,
    /// ISA10, `HHMM`.
    pub time: &'a str,
    /// ISA13, 9 digits.
    pub control_number: &'a str,
    /// ISA15, `T` (test) or `P` (production).
    pub usage: &'a str,
}

/// `GS`: functional group header.
#[derive(Debug, Clone, PartialEq)]
pub struct Gs<'a> {
    /// GS01: `HC` for 837 claims, `HP` for 835 remittances.
    pub functional_id: &'a str,
    /// GS02.
    pub sender: &'a str,
    /// GS03.
    pub receiver: &'a str,
    /// GS06.
    pub control_number: &'a str,
    /// GS08, e.g. `005010X222A1`.
    pub version: &'a str,
}

/// `ST`: transaction set header.
#[derive(Debug, Clone, PartialEq)]
pub struct St<'a> {
    /// ST01: `835` or `837` (others are parsed but rejected later).
    pub transaction_set_id: &'a str,
    /// ST02.
    pub control_number: &'a str,
}

/// `BPR`: financial information (835 only).
#[derive(Debug, Clone, PartialEq)]
pub struct Bpr<'a> {
    /// BPR01, e.g. `I` (remittance information only), `C` (payment + info).
    pub handling_code: &'a str,
    /// BPR02: total payment amount for the whole transaction.
    pub total_payment: f64,
}

/// `NM1`: an individual or organization name.
#[derive(Debug, Clone, PartialEq)]
pub struct Nm1<'a> {
    /// NM101: `85` billing provider, `IL` subscriber, `QC` patient, ...
    pub entity_id: &'a str,
    /// NM102: `1` person, `2` organization.
    pub entity_type: &'a str,
    /// NM103: last name, or organization name.
    pub last_name: &'a str,
    /// NM104: first name (absent for organizations).
    pub first_name: Option<&'a str>,
    /// NM108: what kind of ID NM109 is (`XX` = NPI, `MI` = member ID).
    pub id_qualifier: Option<&'a str>,
    /// NM109.
    pub id: Option<&'a str>,
}

/// `CLM`: claim header (837).
#[derive(Debug, Clone, PartialEq)]
pub struct Clm<'a> {
    /// CLM01: the submitter's claim ID.
    pub claim_id: &'a str,
    /// CLM02: total charge.
    pub total_charge: f64,
}

/// `SV1`: professional service line (837P).
#[derive(Debug, Clone, PartialEq)]
pub struct Sv1<'a> {
    /// First component of composite SV101 (`HC` in `HC:99213`). `None` when
    /// SV101 has no component separator.
    pub qualifier: Option<&'a str>,
    /// Procedure code: second component of SV101 (`99213`), or all of SV101
    /// when it has no component separator.
    pub procedure_code: &'a str,
    /// SV102: line charge.
    pub charge: f64,
    /// SV104: units. Optional, but if present it must be numeric.
    pub units: Option<f64>,
}

/// `CLP`: claim payment information (835).
#[derive(Debug, Clone, PartialEq)]
pub struct Clp<'a> {
    /// CLP01: the claim ID from the original 837.
    pub claim_id: &'a str,
    /// CLP02: status (`1` processed as primary, `4` denied, ...).
    pub status_code: &'a str,
    /// CLP03: total charge.
    pub total_charge: f64,
    /// CLP04: amount paid.
    pub paid: f64,
    /// CLP05: patient responsibility. Optional, numeric if present.
    pub patient_responsibility: Option<f64>,
}

/// `SVC`: service payment information (835).
#[derive(Debug, Clone, PartialEq)]
pub struct Svc<'a> {
    /// First component of composite SVC01, as in [`Sv1::qualifier`].
    pub qualifier: Option<&'a str>,
    /// Procedure code from SVC01, as in [`Sv1::procedure_code`].
    pub procedure_code: &'a str,
    /// SVC02: line charge.
    pub charge: f64,
    /// SVC03: line payment.
    pub paid: f64,
}

/// `SE`: transaction set trailer.
#[derive(Debug, Clone, PartialEq)]
pub struct Se<'a> {
    /// SE01: number of segments from ST to SE inclusive.
    pub segment_count: usize,
    /// SE02: must equal ST02.
    pub control_number: &'a str,
}

/// `GE`: functional group trailer.
#[derive(Debug, Clone, PartialEq)]
pub struct Ge<'a> {
    /// GE01: number of transaction sets in the group.
    pub transaction_count: usize,
    /// GE02: must equal GS06.
    pub control_number: &'a str,
}

/// `IEA`: interchange trailer.
#[derive(Debug, Clone, PartialEq)]
pub struct Iea<'a> {
    /// IEA01: number of functional groups.
    pub group_count: usize,
    /// IEA02: must equal ISA13.
    pub control_number: &'a str,
}

/// Every segment this crate understands, plus a catch-all.
///
/// `Unknown` keeps the raw parts so nothing is lost (`HL`, `DTM`, `CAS`, ...).
#[derive(Debug, Clone, PartialEq)]
pub enum Segment<'a> {
    Isa(Isa<'a>),
    Gs(Gs<'a>),
    St(St<'a>),
    Bpr(Bpr<'a>),
    Nm1(Nm1<'a>),
    Clm(Clm<'a>),
    Sv1(Sv1<'a>),
    Clp(Clp<'a>),
    Svc(Svc<'a>),
    Se(Se<'a>),
    Ge(Ge<'a>),
    Iea(Iea<'a>),
    Unknown { id: &'a str, elements: Vec<&'a str> },
}

impl<'a> Segment<'a> {
    /// Convert one raw segment into a typed segment.
    ///
    /// Takes `raw` by value: for `Unknown` you can move its `elements` Vec in
    /// instead of cloning it. `delims` is needed to split composite elements
    /// (SV101, SVC01) on `delims.component`.
    ///
    /// Errors: [`EdiError::MissingElement`] for an absent/empty required
    /// element, [`EdiError::InvalidNumber`] for a non-numeric amount or count.
    pub fn from_raw(raw: RawSegment<'a>, delims: &Delimiters) -> Result<Segment<'a>, EdiError> {
        todo!("Task 3: match on raw.id and build each variant")
    }

    /// The segment ID (`"CLM"`, `"SE"`, ...). Write this as one exhaustive
    /// `match` with no `_` arm, so adding a variant forces you to update it.
    pub fn id(&self) -> &'a str {
        todo!("Task 3: exhaustive match")
    }
}

/// Return element `index` of `raw`, or [`EdiError::MissingElement`] if it is
/// absent or empty.
pub fn required<'a>(raw: &RawSegment<'a>, index: usize) -> Result<&'a str, EdiError> {
    todo!("Task 3: required-element helper")
}

/// Parse an X12 decimal amount (`"150.00"`, `"-5"`, `"0"`).
///
/// Only an optional leading `-`, digits, and at most one `.` are allowed, with
/// at least one digit. `""`, `"12A.00"`, `"1e5"`, `"NaN"` and `"inf"` are all
/// [`EdiError::InvalidNumber`] (note: `str::parse::<f64>` alone accepts some of
/// these). `segment` and `index` are only used to build the error.
pub fn parse_amount(segment: &str, index: usize, value: &str) -> Result<f64, EdiError> {
    todo!("Task 3: strict decimal parsing")
}

/// Parse a non-negative integer count (SE01, GE01, IEA01).
/// Errors with [`EdiError::InvalidNumber`] like [`parse_amount`].
pub fn parse_count(segment: &str, index: usize, value: &str) -> Result<usize, EdiError> {
    todo!("Task 3: count parsing")
}

/// Tokenize `input` and convert every raw segment with [`Segment::from_raw`],
/// stopping at the first error.
///
/// Needs the delimiters for `from_raw`, so detect them once up front.
pub fn parse_segments<'a>(input: &'a str) -> Result<Vec<Segment<'a>>, EdiError> {
    todo!("Task 4: tokenize -> from_raw, collecting into Result<Vec<_>, _>")
}
