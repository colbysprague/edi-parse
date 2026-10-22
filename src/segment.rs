//! Task 3: turn `RawSegment`s into a typed `Segment` enum.
//! Task 4 (part): `parse_segments`, the tokenize → typed-segments pipeline.
//!
//! Element numbers in the docs below are X12 numbers (`CLM02` = `raw.element(2)`).

use crate::error::EdiError;
use crate::tokenizer::{tokenize_with, Delimiters, RawSegment};
use regex::Regex;
use std::str::FromStr;
use std::sync::LazyLock;
static EDI_MONEY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-?([0-9]+\.?[0-9]*|\.[0-9]+)$").unwrap());

static EDI_COUNT_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]+$").unwrap());

macro_rules! segment_def {
    (
        $(#[$attr:meta])*                      // the /// doc and #[derive(...)] on the struct
        struct $name:ident<$lt:lifetime> {     // struct Clm<'a> {
            $(                                 // for each field:
                $(#[$field_attr:meta])*        //   its /// doc comment
                $field:ident                   //   claim_id
                : $ty:ty                       //   : &'a str
                = $idx:literal                 //   = 1
                $( => $method:ident )?         //   optional: => trim_end
            ),* $(,)?                          // comma-separated, optional trailing comma
        }
    ) => {
        $(#[$attr])*
        pub struct $name<$lt> {
            $(
                $(#[$field_attr])*
                pub $field: $ty,
            )*
        }

        impl<$lt> $name<$lt> {
            pub fn from_segment(raw: &RawSegment<$lt>) -> Result<Self, EdiError> {
                Ok($name {
                    $(
                        $field: element::<$ty>(raw, $idx)? $( .$method() )?,
                    )*
                })
            }
        }

    };
}

segment_def! {
    /// `ISA`: interchange header.
    #[derive(Debug, Clone, PartialEq)]
    struct Isa<'a> {
        /// ISA06, with trailing pad spaces trimmed.
        sender_id: &'a str = 6 => trim_end,
        /// ISA08, with trailing pad spaces trimmed.
        receiver_id: &'a str = 8 => trim_end,
        /// ISA09, `YYMMDD`.
        date: &'a str = 9,
        /// ISA10, `HHMM`.
        time: &'a str = 10,
        /// ISA13, 9 digits.
        control_number: &'a str = 13,
        /// ISA15, `T` (test) or `P` (production).
        usage: &'a str = 15,
    }
}

segment_def! {
    /// `GS`: functional group header.
    #[derive(Debug, Clone, PartialEq)]
    struct Gs<'a> {
        /// GS01: `HC` for 837 claims, `HP` for 835 remittances.
        functional_id: &'a str = 1,
        /// GS02.
        sender: &'a str = 2,
        /// GS03.
        receiver: &'a str = 3,
        /// GS06.
        control_number: &'a str = 6,
        /// GS08, e.g. `005010X222A1`.
        version: &'a str = 8,
    }
}

segment_def! {
    /// `ST`: transaction set header.
    #[derive(Debug, Clone, PartialEq)]
    struct St<'a> {
        /// ST01: `835` or `837` (others are parsed but rejected later).
        transaction_set_id: &'a str = 1,
        /// ST02.
        control_number: &'a str = 2,
    }
}

segment_def! {
    /// `BPR`: financial information (835 only).
    #[derive(Debug, Clone, PartialEq)]
    struct Bpr<'a> {
        /// BPR01, e.g. `I` (remittance information only), `C` (payment + info).
        handling_code: &'a str = 1,
        /// BPR02: total payment amount for the whole transaction.
        total_payment: f64 = 2,
    }
}

segment_def! {
    /// `NM1`: an individual or organization name.
    #[derive(Debug, Clone, PartialEq)]
    struct Nm1<'a> {
        /// NM101: `85` billing provider, `IL` subscriber, `QC` patient, ...
        entity_id: &'a str = 1,
        /// NM102: `1` person, `2` organization.
        entity_type: &'a str = 2,
        /// NM103: last name, or organization name.
        last_name: &'a str = 3,
        /// NM104: first name (absent for organizations).
        first_name: Option<&'a str> = 4,
        /// NM108: what kind of ID NM109 is (`XX` = NPI, `MI` = member ID).
        id_qualifier: Option<&'a str> = 8,
        /// NM109.
        id: Option<&'a str> = 9,
    }

}

segment_def! {
    /// `CLM`: claim header (837).
    #[derive(Debug, Clone, PartialEq)]
    struct Clm<'a> {
        /// CLM01: the submitter's claim ID.
        claim_id: &'a str = 1,
        /// CLM02: total charge.
        total_charge: f64 = 2,
    }
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

impl<'a> Sv1<'a> {
    pub fn from_segment(raw: &RawSegment<'a>, delims: &Delimiters) -> Result<Self, EdiError> {
        let (qualifier, procedure_code) =
            split_composite(element::<&'a str>(raw, 1)?, delims.component);

        Ok(Sv1 {
            qualifier: qualifier,
            procedure_code: procedure_code,
            charge: element::<f64>(raw, 2)?,
            units: element::<Option<f64>>(raw, 4)?,
        })
    }
}

segment_def! {
    /// `CLP`: claim payment information (835).
    #[derive(Debug, Clone, PartialEq)]
    struct Clp<'a> {
        /// CLP01: the claim ID from the original 837.
        claim_id: &'a str = 1,
        /// CLP02: status (`1` processed as primary, `4` denied, ...).
        status_code: &'a str = 2,
        /// CLP03: total charge.
        total_charge: f64 = 3,
        /// CLP04: amount paid.
        paid: f64 = 4,
        /// CLP05: patient responsibility. Optional, numeric if present.
        patient_responsibility: Option<f64> = 5,
    }
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

impl<'a> Svc<'a> {
    pub fn from_segment(raw: &RawSegment<'a>, delims: &Delimiters) -> Result<Self, EdiError> {
        let (qualifier, procedure_code) =
            split_composite(element::<&'a str>(raw, 1)?, delims.component);

        Ok(Svc {
            qualifier: qualifier,
            procedure_code: procedure_code,
            charge: element::<f64>(raw, 2)?,
            paid: element::<f64>(raw, 3)?,
        })
    }
}

segment_def! {
    /// `SE`: transaction set trailer.
    #[derive(Debug, Clone, PartialEq)]
    struct Se<'a> {
        /// SE01: number of segments from ST to SE inclusive.
        segment_count: usize = 1,
        /// SE02: must equal ST02.
        control_number: &'a str = 2,
    }
}

segment_def! {
    /// `GE`: functional group trailer.
    #[derive(Debug, Clone, PartialEq)]
    struct Ge<'a> {
        /// GE01: number of transaction sets in the group.
        transaction_count: usize = 1,
        /// GE02: must equal GS06.
        control_number: &'a str = 2,
    }
}

segment_def! {
    /// `IEA`: interchange trailer.
    #[derive(Debug, Clone, PartialEq)]
    struct Iea<'a> {
        /// IEA01: number of functional groups.
        group_count: usize = 1,
        /// IEA02: must equal ISA13.
        control_number: &'a str = 2 ,
    }
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

pub trait FromElement<'a>: Sized {
    fn from_element(s: &RawSegment<'a>, index: usize) -> Result<Self, EdiError>;
}

impl<'a> FromElement<'a> for f64 {
    fn from_element(s: &RawSegment<'a>, index: usize) -> Result<Self, EdiError> {
        parse_amount(s.id, index, required(s, index)?)
    }
}

impl<'a> FromElement<'a> for usize {
    fn from_element(s: &RawSegment<'a>, index: usize) -> Result<Self, EdiError> {
        parse_count(s.id, index, required(s, index)?)
    }
}

impl<'a> FromElement<'a> for &'a str {
    fn from_element(s: &RawSegment<'a>, index: usize) -> Result<Self, EdiError> {
        required(s, index)
    }
}

impl<'a, T: FromElement<'a>> FromElement<'a> for Option<T> {
    fn from_element(s: &RawSegment<'a>, index: usize) -> Result<Self, EdiError> {
        match s.element(index) {
            None => Ok(None),
            Some(_) => T::from_element(s, index).map(Some),
        }
    }
}

pub fn element<'a, T: FromElement<'a>>(s: &RawSegment<'a>, index: usize) -> Result<T, EdiError> {
    T::from_element(s, index)
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
        match raw.id {
            "ISA" => Ok(Segment::Isa(Isa::from_segment(&raw)?)),
            "GS" => Ok(Segment::Gs(Gs::from_segment(&raw)?)),
            "ST" => Ok(Segment::St(St::from_segment(&raw)?)),
            "BPR" => Ok(Segment::Bpr(Bpr::from_segment(&raw)?)),
            "NM1" => Ok(Segment::Nm1(Nm1::from_segment(&raw)?)),
            "CLM" => Ok(Segment::Clm(Clm::from_segment(&raw)?)),
            "SV1" => Ok(Segment::Sv1(Sv1::from_segment(&raw, &delims)?)),
            "CLP" => Ok(Segment::Clp(Clp::from_segment(&raw)?)),
            "SVC" => Ok(Segment::Svc(Svc::from_segment(&raw, &delims)?)),
            "SE" => Ok(Segment::Se(Se::from_segment(&raw)?)),
            "GE" => Ok(Segment::Ge(Ge::from_segment(&raw)?)),
            "IEA" => Ok(Segment::Iea(Iea::from_segment(&raw)?)),
            _ => Ok(Segment::Unknown {
                id: raw.id,
                elements: raw.elements,
            }),
        }
    }

    /// The segment ID (`"CLM"`, `"SE"`, ...). Write this as one exhaustive
    /// `match` with no `_` arm, so adding a variant forces you to update it.
    pub fn id(&self) -> &'a str {
        match self {
            Segment::Isa(_) => "ISA",
            Segment::Gs(_) => "GS",
            Segment::St(_) => "ST",
            Segment::Bpr(_) => "BPR",
            Segment::Nm1(_) => "NM1",
            Segment::Clm(_) => "CLM",
            Segment::Sv1(_) => "SV1",
            Segment::Clp(_) => "CLP",
            Segment::Svc(_) => "SVC",
            Segment::Se(_) => "SE",
            Segment::Ge(_) => "GE",
            Segment::Iea(_) => "IEA",
            Segment::Unknown { id, .. } => *id,
        }
    }
}

/// Return element `index` of `raw`, or [`EdiError::MissingElement`] if it is
/// absent or empty.
pub fn required<'a>(raw: &RawSegment<'a>, index: usize) -> Result<&'a str, EdiError> {
    match raw.element(index) {
        Some(e) => Ok(e),
        None => Err(EdiError::MissingElement {
            segment: (raw.id.to_string()),
            index,
        }),
    }
}

/// Parse an X12 decimal amount (`"150.00"`, `"-5"`, `"0"`).
///
/// Only an optional leading `-`, digits, and at most one `.` are allowed, with
/// at least one digit. `""`, `"12A.00"`, `"1e5"`, `"NaN"` and `"inf"` are all
/// [`EdiError::InvalidNumber`] (note: `str::parse::<f64>` alone accepts some of
/// these). `segment` and `index` are only used to build the error.
pub fn parse_amount(segment: &str, index: usize, value: &str) -> Result<f64, EdiError> {
    parse_to::<f64>(segment, index, value, &EDI_MONEY_PATTERN)
}

/// Parse value from a given segment index to type T using the supplied Regex
fn parse_to<T: FromStr>(
    segment: &str,
    index: usize,
    value: &str,
    pattern: &Regex,
) -> Result<T, EdiError> {
    Some(value)
        .filter(|v| pattern.is_match(v))
        .and_then(|v| v.parse::<T>().ok())
        .ok_or_else(|| EdiError::InvalidNumber {
            segment: segment.to_string(),
            index,
            value: value.to_string(),
        })
}

/// Parse a non-negative integer count (SE01, GE01, IEA01).
/// Errors with [`EdiError::InvalidNumber`] like [`parse_amount`].
pub fn parse_count(segment: &str, index: usize, value: &str) -> Result<usize, EdiError> {
    parse_to::<usize>(segment, index, value, &EDI_COUNT_PATTERN)
}

/// Tokenize `input` and convert every raw segment with [`Segment::from_raw`],
/// stopping at the first error.
///
/// Needs the delimiters for `from_raw`, so detect them once up front.
pub fn parse_segments<'a>(input: &'a str) -> Result<Vec<Segment<'a>>, EdiError> {
    let delims = Delimiters::detect(input)?;
    tokenize_with(input, &delims)?
        .into_iter()
        .map(|segment| Segment::from_raw(segment, &delims))
        .collect()
}

fn split_composite(value: &str, sep: char) -> (Option<&str>, &str) {
    match value.split_once(sep) {
        Some((qualifier, code)) => (Some(qualifier), code),
        None => (None, value),
    }
}
