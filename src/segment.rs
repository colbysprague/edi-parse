//! Turn `RawSegment`s into a typed `Segment` enum, and `parse_segments`, the
//! tokenize → typed-segments pipeline.
//!
//! Element numbers in the docs below are X12 numbers (`CLM02` = `raw.element(2)`).

use crate::error::EdiError;
use crate::segment_id;
use crate::tokenizer::{tokenize_with, Delimiters, RawSegment};
use std::str::FromStr;

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
        /// NM103: last name, or organization name. X12 allows it to be blank,
        /// for example when `NM109` identifies the party on its own.
        last_name: Option<&'a str> = 3,
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

/// A composite procedure identifier, such as SV101, SV202, SV301 or SVC01.
///
/// ```text
/// HC:K0001:RR:KH:BR                HC:J3490:::::Hep Lock see NDC#
/// 1  2     3  4  5                 1  2     3-6 7
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Procedure<'a> {
    /// Component 1 (`HC`). `None` when the element has no component separator.
    pub qualifier: Option<&'a str>,
    /// Component 2 (`K0001`), or the whole element when it has no component
    /// separator.
    pub code: &'a str,
    /// Components 3-6, with empty positions left out.
    pub modifiers: Vec<&'a str>,
    /// Component 7, if present.
    pub description: Option<&'a str>,
}

impl<'a> Procedure<'a> {
    /// Split a composite on `separator`, the file's component separator.
    pub fn parse(value: &'a str, separator: char) -> Self {
        let Some((qualifier, rest)) = value.split_once(separator) else {
            return Procedure {
                qualifier: None,
                code: value,
                modifiers: Vec::new(),
                description: None,
            };
        };

        let mut components = rest.split(separator);
        let code = components.next().unwrap_or_default();
        let modifiers = components
            .by_ref()
            .take(4)
            .filter(|modifier| !modifier.is_empty())
            .collect();
        let description = components.next().filter(|text| !text.is_empty());

        Procedure {
            qualifier: Some(qualifier),
            code,
            modifiers,
            description,
        }
    }
}

/// `SV1`: professional service line (837P).
#[derive(Debug, Clone, PartialEq)]
pub struct Sv1<'a> {
    /// SV101.
    pub procedure: Procedure<'a>,
    /// SV102: line charge.
    pub charge: f64,
    /// SV104: units. Optional, but if present it must be numeric.
    pub units: Option<f64>,
}

impl<'a> Sv1<'a> {
    pub fn from_segment(raw: &RawSegment<'a>, delims: &Delimiters) -> Result<Self, EdiError> {
        Ok(Sv1 {
            procedure: Procedure::parse(element::<&'a str>(raw, 1)?, delims.component),
            charge: element::<f64>(raw, 2)?,
            units: element::<Option<f64>>(raw, 4)?,
        })
    }
}

/// `SV2`: institutional service line (837I).
#[derive(Debug, Clone, PartialEq)]
pub struct Sv2<'a> {
    /// SV201: revenue code.
    pub revenue_code: Option<&'a str>,
    /// SV202. Situational: a line can have only a revenue code.
    pub procedure: Option<Procedure<'a>>,
    /// SV203: line charge.
    pub charge: f64,
}

impl<'a> Sv2<'a> {
    pub fn from_segment(raw: &RawSegment<'a>, delims: &Delimiters) -> Result<Self, EdiError> {
        Ok(Sv2 {
            revenue_code: element::<Option<&'a str>>(raw, 1)?,
            procedure: element::<Option<&'a str>>(raw, 2)?
                .map(|value| Procedure::parse(value, delims.component)),
            charge: element::<f64>(raw, 3)?,
        })
    }
}

/// `SV3`: dental service line (837D).
#[derive(Debug, Clone, PartialEq)]
pub struct Sv3<'a> {
    /// SV301.
    pub procedure: Procedure<'a>,
    /// SV302: line charge.
    pub charge: f64,
}

impl<'a> Sv3<'a> {
    pub fn from_segment(raw: &RawSegment<'a>, delims: &Delimiters) -> Result<Self, EdiError> {
        Ok(Sv3 {
            procedure: Procedure::parse(element::<&'a str>(raw, 1)?, delims.component),
            charge: element::<f64>(raw, 2)?,
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
    /// SVC01.
    pub procedure: Procedure<'a>,
    /// SVC02: line charge.
    pub charge: f64,
    /// SVC03: line payment.
    pub paid: f64,
}

impl<'a> Svc<'a> {
    pub fn from_segment(raw: &RawSegment<'a>, delims: &Delimiters) -> Result<Self, EdiError> {
        Ok(Svc {
            procedure: Procedure::parse(element::<&'a str>(raw, 1)?, delims.component),
            charge: element::<f64>(raw, 2)?,
            paid: element::<f64>(raw, 3)?,
        })
    }
}

/// One reason code, amount and quantity from a `CAS` segment.
#[derive(Debug, Clone, PartialEq)]
pub struct CasTriplet<'a> {
    /// CAS02, 05, 08, 11, 14 or 17.
    pub reason_code: &'a str,
    /// CAS03, 06, 09, 12, 15 or 18.
    pub amount: f64,
    /// CAS04, 07, 10, 13, 16 or 19. Optional, numeric if present.
    pub quantity: Option<f64>,
}

/// `CAS`: claim or service line adjustment (835).
#[derive(Debug, Clone, PartialEq)]
pub struct Cas<'a> {
    /// CAS01: `CO`, `PR`, `OA`, `PI` or `CR`.
    pub group_code: &'a str,
    /// CAS02 to CAS19: 1 to 6 triplets. Blank triplets are left out.
    pub triplets: Vec<CasTriplet<'a>>,
}

impl<'a> Cas<'a> {
    pub fn from_segment(raw: &RawSegment<'a>) -> Result<Self, EdiError> {
        let group_code = element::<&'a str>(raw, 1)?;
        if !matches!(group_code, "CO" | "PR" | "OA" | "PI" | "CR") {
            return Err(EdiError::InvalidGroupCode {
                value: group_code.to_string(),
            });
        }

        let mut triplets = Vec::new();
        for first in (2..=17).step_by(3) {
            // The first triplet is required, so it always falls through to the
            // checks below. A later one is either wholly blank or must be valid.
            if first > 2 && (first..first + 3).all(|index| raw.element(index).is_none()) {
                continue;
            }

            triplets.push(CasTriplet {
                reason_code: element::<&'a str>(raw, first)?,
                amount: element::<f64>(raw, first + 1)?,
                quantity: element::<Option<f64>>(raw, first + 2)?,
            });
        }

        Ok(Cas {
            group_code,
            triplets,
        })
    }
}

segment_def! {
    /// `LQ`: health care remark code (835).
    #[derive(Debug, Clone, PartialEq)]
    struct Lq<'a> {
        /// LQ01: `HE` (RARC) or `RX` (NCPDP reject code) in an 835.
        qualifier: &'a str = 1,
        /// LQ02.
        code: &'a str = 2,
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
/// `Unknown` keeps the raw parts so nothing is lost (`HL`, `DTM`, `REF`, ...).
#[derive(Debug, Clone, PartialEq)]
pub enum Segment<'a> {
    Isa(Isa<'a>),
    Gs(Gs<'a>),
    St(St<'a>),
    Bpr(Bpr<'a>),
    Nm1(Nm1<'a>),
    Clm(Clm<'a>),
    Sv1(Sv1<'a>),
    Sv2(Sv2<'a>),
    Sv3(Sv3<'a>),
    Clp(Clp<'a>),
    Svc(Svc<'a>),
    Cas(Cas<'a>),
    Lq(Lq<'a>),
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
    /// Takes `raw` by value, so `Unknown` can keep its `elements` Vec without
    /// cloning it. `delims` is needed to split composite elements
    /// (SV101, SV202, SV301, SVC01) on `delims.component`.
    ///
    /// Errors: [`EdiError::MissingElement`] for an absent/empty required
    /// element, [`EdiError::InvalidNumber`] for a non-numeric amount or count,
    /// [`EdiError::InvalidGroupCode`] for a `CAS01` outside the allowed codes.
    pub fn from_raw(raw: RawSegment<'a>, delims: &Delimiters) -> Result<Segment<'a>, EdiError> {
        // Arms must be paths (`segment_id::ISA`), not bare names: a bare `ISA`
        // that isn't a const in scope is a catch-all binding, not a comparison.
        match raw.id {
            segment_id::ISA => Ok(Segment::Isa(Isa::from_segment(&raw)?)),
            segment_id::GS => Ok(Segment::Gs(Gs::from_segment(&raw)?)),
            segment_id::ST => Ok(Segment::St(St::from_segment(&raw)?)),
            segment_id::BPR => Ok(Segment::Bpr(Bpr::from_segment(&raw)?)),
            segment_id::NM1 => Ok(Segment::Nm1(Nm1::from_segment(&raw)?)),
            segment_id::CLM => Ok(Segment::Clm(Clm::from_segment(&raw)?)),
            segment_id::SV1 => Ok(Segment::Sv1(Sv1::from_segment(&raw, delims)?)),
            segment_id::SV2 => Ok(Segment::Sv2(Sv2::from_segment(&raw, delims)?)),
            segment_id::SV3 => Ok(Segment::Sv3(Sv3::from_segment(&raw, delims)?)),
            segment_id::CLP => Ok(Segment::Clp(Clp::from_segment(&raw)?)),
            segment_id::SVC => Ok(Segment::Svc(Svc::from_segment(&raw, delims)?)),
            segment_id::CAS => Ok(Segment::Cas(Cas::from_segment(&raw)?)),
            segment_id::LQ => Ok(Segment::Lq(Lq::from_segment(&raw)?)),
            segment_id::SE => Ok(Segment::Se(Se::from_segment(&raw)?)),
            segment_id::GE => Ok(Segment::Ge(Ge::from_segment(&raw)?)),
            segment_id::IEA => Ok(Segment::Iea(Iea::from_segment(&raw)?)),
            _ => Ok(Segment::Unknown {
                id: raw.id,
                elements: raw.elements,
            }),
        }
    }

    /// The segment ID (`"CLM"`, `"SE"`, ...).
    pub fn id(&self) -> &'a str {
        match self {
            Segment::Isa(_) => segment_id::ISA,
            Segment::Gs(_) => segment_id::GS,
            Segment::St(_) => segment_id::ST,
            Segment::Bpr(_) => segment_id::BPR,
            Segment::Nm1(_) => segment_id::NM1,
            Segment::Clm(_) => segment_id::CLM,
            Segment::Sv1(_) => segment_id::SV1,
            Segment::Sv2(_) => segment_id::SV2,
            Segment::Sv3(_) => segment_id::SV3,
            Segment::Clp(_) => segment_id::CLP,
            Segment::Svc(_) => segment_id::SVC,
            Segment::Cas(_) => segment_id::CAS,
            Segment::Lq(_) => segment_id::LQ,
            Segment::Se(_) => segment_id::SE,
            Segment::Ge(_) => segment_id::GE,
            Segment::Iea(_) => segment_id::IEA,
            Segment::Unknown { id, .. } => id,
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
    parse_to::<f64>(segment, index, value, is_amount)
}

/// One or more ASCII digits.
fn is_count(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

/// An optional `-`, then digits with at most one `.`, and at least one digit.
fn is_amount(value: &str) -> bool {
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    (!whole.is_empty() || !fraction.is_empty())
        && whole.bytes().all(|b| b.is_ascii_digit())
        && fraction.bytes().all(|b| b.is_ascii_digit())
}

/// Parse value from a given segment index to type T if `is_valid` accepts it.
fn parse_to<T: FromStr>(
    segment: &str,
    index: usize,
    value: &str,
    is_valid: fn(&str) -> bool,
) -> Result<T, EdiError> {
    Some(value)
        .filter(|v| is_valid(v))
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
    parse_to::<usize>(segment, index, value, is_count)
}

/// Tokenize `input` and convert every raw segment with [`Segment::from_raw`],
/// stopping at the first error.
///
/// Needs the delimiters for `from_raw`, so detect them once up front.
pub fn parse_segments<'a>(input: &'a str) -> Result<Vec<Segment<'a>>, EdiError> {
    let delims = Delimiters::detect(input)?;
    let raw = tokenize_with(input, &delims)?;
    let mut out = Vec::with_capacity(raw.len());
    for segment in raw {
        out.push(Segment::from_raw(segment, &delims)?);
    }
    Ok(out)
}
