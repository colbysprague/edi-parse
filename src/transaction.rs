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
use crate::parse_segments;
use crate::segment::Segment;
use crate::segment_id::{self};

mod claim;
mod interchange;
mod party;
mod remit;
mod service_line;

pub use claim::{Claim, ClaimSet};
pub use interchange::{Interchange, Transaction};
pub use party::Party;
pub use remit::{Remit, Remittance};
pub use service_line::ServiceLine;

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
    let Some(Segment::Isa(_)) = segments.first() else {
        return Err(EdiError::MissingIsa);
    };
    let Some(Segment::Iea(_)) = segments.last() else {
        return Err(EdiError::MissingIea);
    };

    let unexpected = |i: usize, expected: &str, found: &str| EdiError::UnexpectedSegment {
        position: i,
        expected: expected.to_string(),
        found: found.to_string(),
    };

    let mut open = None;

    for (i, seg) in segments.iter().enumerate() {
        match seg {
            Segment::St(st) => {
                if open.is_some() {
                    return Err(unexpected(i, segment_id::SE, seg.id()));
                }
                open = Some((i, st));
            }
            Segment::Se(se) => {
                let Some((start, st)) = open.take() else {
                    return Err(unexpected(i, segment_id::ST, seg.id()));
                };

                let actual = i - start + 1;
                if se.segment_count != actual {
                    return Err(EdiError::SegmentCountMismatch {
                        control_number: se.control_number.to_string(),
                        declared: se.segment_count,
                        actual,
                    });
                }

                if se.control_number != st.control_number {
                    return Err(EdiError::ControlNumberMismatch {
                        segment: segment_id::SE.to_string(),
                        expected: st.control_number.to_string(),
                        found: se.control_number.to_string(),
                    });
                }
            }
            Segment::Ge(_) | Segment::Iea(_) if open.is_some() => {
                return Err(unexpected(i, segment_id::SE, seg.id()));
            }
            _ => {}
        }
    }

    Ok(())
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
    let mut header = None;
    let mut transactions = Vec::new();
    let mut billing_provider = None;
    let mut subscriber = None;

    for (i, seg) in segments.into_iter().enumerate() {
        match seg {
            Segment::Isa(isa) => header = Some(isa),
            Segment::St(st) => match st.transaction_set_id {
                "837" => transactions.push(Transaction::Claims(ClaimSet {
                    control_number: (st.control_number),
                    claims: (Vec::new()),
                })),
                "835" => transactions.push(Transaction::Remittance(Remittance {
                    control_number: (st.control_number),
                    total_payment: (0.0),
                    remits: (Vec::new()),
                })),
                _ => {
                    return Err(EdiError::UnsupportedTransaction(
                        st.transaction_set_id.to_string(),
                    ))
                }
            },
            Segment::Nm1(nm1) => {
                let party = Party {
                    first_name: nm1.first_name,
                    last_name: nm1.last_name,
                    id: nm1.id,
                };
                match nm1.entity_id {
                    "85" => billing_provider = Some(party),
                    "IL" => subscriber = Some(party),
                    "QC" => {
                        let unexpected = unexpected();

                        let Some(Transaction::Remittance(set)) = transactions.last_mut() else {
                            return Err(unexpected(i, segment_id::CLP, segment_id::SVC));
                        };

                        let Some(remit) = set.remits.last_mut() else {
                            return Err(unexpected(i, segment_id::CLP, segment_id::SVC));
                        };

                        remit.patient = Some(party);
                    }
                    _ => {}
                }
            }
            Segment::Clm(clm) => {
                if let Some(Transaction::Claims(set)) = transactions.last_mut() {
                    set.claims.push(Claim {
                        claim_id: (clm.claim_id),
                        total_charge: (clm.total_charge),
                        billing_provider: billing_provider.clone(),
                        subscriber: subscriber.clone(),
                        service_lines: (Vec::new()),
                    })
                }
            }
            Segment::Sv1(sv1) => {
                let unexpected = unexpected();

                let Some(Transaction::Claims(set)) = transactions.last_mut() else {
                    return Err(unexpected(i, segment_id::CLM, segment_id::SV1));
                };

                let Some(claim) = set.claims.last_mut() else {
                    return Err(unexpected(i, segment_id::CLM, segment_id::SV1));
                };

                claim.service_lines.push(ServiceLine {
                    procedure_code: (sv1.procedure_code),
                    charge: (sv1.charge),
                    paid: (None),
                })
            }
            Segment::Bpr(bpr) => {
                if let Some(Transaction::Remittance(set)) = transactions.last_mut() {
                    set.total_payment = bpr.total_payment
                }
            }
            Segment::Clp(clp) => {
                if let Some(Transaction::Remittance(set)) = transactions.last_mut() {
                    set.remits.push(Remit {
                        claim_id: (clp.claim_id),
                        status_code: (clp.status_code),
                        total_charge: (clp.total_charge),
                        paid: (clp.paid),
                        patient_responsibility: (clp.patient_responsibility),
                        patient: (None),
                        service_lines: (Vec::new()),
                    })
                }
            }
            Segment::Svc(svc) => {
                let unexpected = unexpected();

                let Some(Transaction::Remittance(set)) = transactions.last_mut() else {
                    return Err(unexpected(i, segment_id::CLP, segment_id::SVC));
                };

                let Some(remit) = set.remits.last_mut() else {
                    return Err(unexpected(i, segment_id::CLP, segment_id::SVC));
                };

                remit.service_lines.push(ServiceLine {
                    procedure_code: (svc.procedure_code),
                    charge: (svc.charge),
                    paid: (Some(svc.paid)),
                })
            }
            _ => {}
        }
    }

    // write the interchange to the return value
    let isa = header.unwrap();
    Ok(Interchange {
        sender_id: isa.sender_id.trim(),
        receiver_id: isa.receiver_id.trim(),
        control_number: isa.control_number,
        transactions,
    })
}

fn unexpected() -> impl Fn(usize, &str, &str) -> EdiError {
    let unexpected = |i: usize, expected: &str, found: &str| EdiError::UnexpectedSegment {
        position: i,
        expected: expected.to_string(),
        found: found.to_string(),
    };
    unexpected
}

/// Parse a whole file: `parse_segments` → `check_envelope` → `build_interchange`.
/// Three fallible steps; this should be three lines and three `?`s.
pub fn parse_str<'a>(input: &'a str) -> Result<Interchange<'a>, EdiError> {
    let segments = parse_segments(input)?;
    check_envelope(&segments)?;
    return build_interchange(segments);
}

/// Serialize an interchange to JSON (pretty-printed if `pretty`).
///
/// Expected shape (see `tests/transaction_tests.rs` for the exact checks):
/// - field names exactly as the Rust fields (`claim_id`, `service_lines`, ...);
/// - each transaction has `"type": "claims"` or `"type": "remittance"` next to
///   its own fields (an internally tagged enum);
/// - `Option` fields that are `None` are omitted, not `null`.
pub fn to_json(interchange: &Interchange<'_>, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(interchange)
    } else {
        serde_json::to_string(interchange)
    }
}
