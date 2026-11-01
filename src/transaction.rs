//! Envelope checks (`check_envelope`, `parse_str`), grouping segments into
//! claims/remits, and JSON serialization.
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

use crate::error::EdiError;
use crate::parse_segments;
use crate::segment::Segment;
use crate::segment_id::{self};

mod adjustment;
mod claim;
mod interchange;
mod party;
mod remit;
mod service_line;

pub use adjustment::{Adjustment, Remark};
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

/// What an 835 `CAS` or `LQ` would attach to at the current position.
#[derive(Clone, Copy)]
enum Open {
    /// Outside any transaction, or in an 835 before its first `CLP`.
    Nothing,
    /// In an 837, where `CAS` and `LQ` are valid but not modeled.
    ClaimSet,
    /// The last `CLP` of the current 835.
    Remit,
    /// The last `SVC` of the current remit.
    Line,
}

/// Group a checked segment list into an [`Interchange`].
///
/// Takes the `Vec` by value, so each segment's data moves into the result.
///
/// Rules:
/// - `ST01` of `837` → [`Transaction::Claims`]; `835` → [`Transaction::Remittance`];
///   anything else → [`EdiError::UnsupportedTransaction`].
/// - 837: `NM1*85` sets the current billing provider, `NM1*IL` the current
///   subscriber, `NM1*QC` the current patient; each `CLM` starts a claim
///   carrying copies of all three; `SV1`, `SV2` or `SV3` adds a line to the
///   current claim.
///   The patient applies to every `CLM` until the next `NM1*QC`, `NM1*IL`, or
///   `ST`, so one subscriber's patient never reaches another's claims.
/// - 835: `BPR02` is the transaction's total payment (0.0 if there is no
///   `BPR`); each `CLP` starts a
///   remit; the `NM1*QC` after it is its patient; `SVC` adds a line.
///   `CAS` and `LQ` attach to whichever of the remit or a line was opened most
///   recently: after a `CLP` they are claim-level adjustments and remarks on
///   the remit, after an `SVC` they belong to that line.
/// - An `SV1`/`SV2`/`SV3`/`SVC` with no open claim/remit, an `NM1*QC` with no
///   open transaction or remit, or a `CAS`/`LQ` with no open remit or line →
///   [`EdiError::UnexpectedSegment`]. `ST` and `SE` close any open remit and
///   line.
/// - All other segments (`HL`, `DTM`, other `NM1`s, ...) are ignored, including
///   `CAS` and `LQ` in an 837, where they carry COB and form data.
pub fn build_interchange<'a>(segments: Vec<Segment<'a>>) -> Result<Interchange<'a>, EdiError> {
    let mut header = None;
    let mut transactions = Vec::new();
    let mut billing_provider = None;
    let mut subscriber = None;
    let mut patient = None;
    let mut open = Open::Nothing;

    for (i, seg) in segments.into_iter().enumerate() {
        match seg {
            Segment::Isa(isa) => header = Some(isa),
            Segment::St(st) => {
                patient = None;

                match st.transaction_set_id {
                    "837" => {
                        open = Open::ClaimSet;
                        transactions.push(Transaction::Claims(ClaimSet {
                            control_number: (st.control_number),
                            claims: (Vec::new()),
                        }))
                    }
                    "835" => {
                        open = Open::Nothing;
                        transactions.push(Transaction::Remittance(Remittance {
                            control_number: (st.control_number),
                            total_payment: (0.0),
                            remits: (Vec::new()),
                        }))
                    }
                    _ => {
                        return Err(EdiError::UnsupportedTransaction(
                            st.transaction_set_id.to_string(),
                        ))
                    }
                }
            }
            Segment::Nm1(nm1) => match nm1.entity_id {
                "85" => billing_provider = Some(Party::from_nm1(&nm1)?),
                "IL" => {
                    subscriber = Some(Party::from_nm1(&nm1)?);
                    patient = None;
                }
                "QC" => {
                    let unexpected = unexpected();

                    match transactions.last_mut() {
                        Some(Transaction::Claims(_)) => patient = Some(Party::from_nm1(&nm1)?),
                        Some(Transaction::Remittance(set)) => {
                            let Some(remit) = set.remits.last_mut() else {
                                return Err(unexpected(i, segment_id::CLP, segment_id::NM1));
                            };

                            remit.patient = Some(Party::from_nm1(&nm1)?);
                        }
                        None => return Err(unexpected(i, segment_id::CLP, segment_id::NM1)),
                    }
                }
                _ => {}
            },
            Segment::Clm(clm) => {
                if let Some(Transaction::Claims(set)) = transactions.last_mut() {
                    set.claims.push(Claim {
                        claim_id: (clm.claim_id),
                        total_charge: (clm.total_charge),
                        billing_provider: billing_provider.clone(),
                        subscriber: subscriber.clone(),
                        patient: patient.clone(),
                        service_lines: (Vec::new()),
                    })
                }
            }
            Segment::Sv1(sv1) => {
                let Some(claim) = open_claim(&mut transactions) else {
                    return Err(unexpected()(i, segment_id::CLM, segment_id::SV1));
                };

                claim
                    .service_lines
                    .push(ServiceLine::new(Some(sv1.procedure), sv1.charge, None))
            }
            Segment::Sv2(sv2) => {
                let Some(claim) = open_claim(&mut transactions) else {
                    return Err(unexpected()(i, segment_id::CLM, segment_id::SV2));
                };

                claim.service_lines.push(ServiceLine {
                    revenue_code: sv2.revenue_code,
                    ..ServiceLine::new(sv2.procedure, sv2.charge, None)
                })
            }
            Segment::Sv3(sv3) => {
                let Some(claim) = open_claim(&mut transactions) else {
                    return Err(unexpected()(i, segment_id::CLM, segment_id::SV3));
                };

                claim
                    .service_lines
                    .push(ServiceLine::new(Some(sv3.procedure), sv3.charge, None))
            }
            Segment::Bpr(bpr) => {
                if let Some(Transaction::Remittance(set)) = transactions.last_mut() {
                    set.total_payment = bpr.total_payment
                }
            }
            Segment::Clp(clp) => {
                if let Some(Transaction::Remittance(set)) = transactions.last_mut() {
                    open = Open::Remit;
                    set.remits.push(Remit {
                        claim_id: (clp.claim_id),
                        status_code: (clp.status_code),
                        total_charge: (clp.total_charge),
                        paid: (clp.paid),
                        patient_responsibility: (clp.patient_responsibility),
                        patient: (None),
                        adjustments: (Vec::new()),
                        remarks: (Vec::new()),
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

                remit.service_lines.push(ServiceLine::new(
                    Some(svc.procedure),
                    svc.charge,
                    Some(svc.paid),
                ));
                open = Open::Line;
            }
            Segment::Cas(cas) => {
                let adjustments = match open {
                    Open::ClaimSet => continue,
                    Open::Nothing => None,
                    Open::Remit => open_remit(&mut transactions).map(|r| &mut r.adjustments),
                    Open::Line => open_line(&mut transactions).map(|l| &mut l.adjustments),
                };
                let Some(adjustments) = adjustments else {
                    return Err(unexpected()(i, segment_id::CLP, segment_id::CAS));
                };

                adjustments.extend(Adjustment::from_cas(cas));
            }
            Segment::Lq(lq) => {
                let remarks = match open {
                    Open::ClaimSet => continue,
                    Open::Nothing => None,
                    Open::Remit => open_remit(&mut transactions).map(|r| &mut r.remarks),
                    Open::Line => open_line(&mut transactions).map(|l| &mut l.remarks),
                };
                let Some(remarks) = remarks else {
                    return Err(unexpected()(i, segment_id::CLP, segment_id::LQ));
                };

                remarks.push(Remark::from(lq));
            }
            Segment::Se(_) => open = Open::Nothing,
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

/// The claim that a service line belongs to, if there is one.
fn open_claim<'s, 'a>(transactions: &'s mut [Transaction<'a>]) -> Option<&'s mut Claim<'a>> {
    match transactions.last_mut() {
        Some(Transaction::Claims(set)) => set.claims.last_mut(),
        _ => None,
    }
}

/// The remit that a `CAS` or `LQ` belongs to, if there is one.
fn open_remit<'s, 'a>(transactions: &'s mut [Transaction<'a>]) -> Option<&'s mut Remit<'a>> {
    match transactions.last_mut() {
        Some(Transaction::Remittance(set)) => set.remits.last_mut(),
        _ => None,
    }
}

/// The 835 service line that a `CAS` or `LQ` belongs to, if there is one.
fn open_line<'s, 'a>(transactions: &'s mut [Transaction<'a>]) -> Option<&'s mut ServiceLine<'a>> {
    open_remit(transactions).and_then(|remit| remit.service_lines.last_mut())
}

fn unexpected() -> impl Fn(usize, &str, &str) -> EdiError {
    |i: usize, expected: &str, found: &str| EdiError::UnexpectedSegment {
        position: i,
        expected: expected.to_string(),
        found: found.to_string(),
    }
}

/// Parse a whole file: `parse_segments` → `check_envelope` → `build_interchange`.
pub fn parse_str<'a>(input: &'a str) -> Result<Interchange<'a>, EdiError> {
    let segments = parse_segments(input)?;
    check_envelope(&segments)?;
    build_interchange(segments)
}

/// Serialize an interchange to JSON (pretty-printed if `pretty`).
///
/// Expected shape (see `tests/transaction_tests.rs` for the exact checks):
/// - field names exactly as the Rust fields (`claim_id`, `service_lines`, ...);
/// - each transaction has `"type": "claims"` or `"type": "remittance"` next to
///   its own fields (an internally tagged enum);
/// - `Option` fields that are `None` are omitted, not `null`;
/// - empty `adjustments` and `remarks` are omitted, so 837 output never has them.
pub fn to_json(interchange: &Interchange<'_>, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(interchange)
    } else {
        serde_json::to_string(interchange)
    }
}
