//! Task 6: the `Validate` trait and its impls.
//!
//! Most tests build structs by hand so they don't depend on the parser.

mod common;

use common::*;
use edi_parse::{
    parse_str, validate_all, Claim, ClaimSet, Interchange, Remit, Remittance, ServiceLine,
    Transaction, Validate, ValidationError,
};

fn line(charge: f64, paid: Option<f64>) -> ServiceLine<'static> {
    ServiceLine {
        procedure_code: "99213",
        modifiers: Vec::new(),
        description: None,
        charge,
        paid,
    }
}

fn claim(id: &'static str, total: f64, lines: &[f64]) -> Claim<'static> {
    Claim {
        claim_id: id,
        total_charge: total,
        billing_provider: None,
        subscriber: None,
        patient: None,
        service_lines: lines.iter().map(|&c| line(c, None)).collect(),
    }
}

fn remit(
    id: &'static str,
    status: &'static str,
    charge: f64,
    paid: f64,
    lines: &[(f64, f64)],
) -> Remit<'static> {
    Remit {
        claim_id: id,
        status_code: status,
        total_charge: charge,
        paid,
        patient_responsibility: None,
        patient: None,
        service_lines: lines.iter().map(|&(c, p)| line(c, Some(p))).collect(),
    }
}

fn errors<T: Validate>(item: &T) -> Vec<ValidationError> {
    item.validate().expect_err("expected validation errors")
}

// ---- Claim ------------------------------------------------------------------

#[test]
fn valid_claim() {
    let c = claim("C1", 150.0, &[100.0, 50.0]);
    assert_eq!(c.validate(), Ok(()));
    assert!(c.is_valid());
}

#[test]
fn claim_charge_mismatch() {
    let errs = errors(&claim("C1", 160.0, &[100.0, 50.0]));
    assert_eq!(errs.len(), 1);
    match &errs[0] {
        ValidationError::ChargeMismatch {
            claim_id,
            declared,
            computed,
        } => {
            assert_eq!(claim_id, "C1");
            assert_approx(*declared, 160.0);
            assert_approx(*computed, 150.0);
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn claim_total_below_line_sum_is_a_mismatch() {
    let errs = errors(&claim("C1", 100.0, &[100.0, 50.0]));
    assert_eq!(
        errs,
        vec![ValidationError::ChargeMismatch {
            claim_id: "C1".into(),
            declared: 100.0,
            computed: 150.0,
        }]
    );
}

#[test]
fn claim_negative_line_counts_toward_the_sum() {
    let errs = errors(&claim("C1", 40.0, &[50.0, -10.0]));
    assert_eq!(
        errs,
        vec![ValidationError::NegativeAmount {
            claim_id: "C1".into(),
            field: "line_charge",
            value: -10.0,
        }]
    );
}

#[test]
fn claim_amounts_use_a_tolerance() {
    // 0.1 + 0.2 != 0.3 in floating point; validation must not care.
    assert_eq!(claim("C1", 0.3, &[0.1, 0.2]).validate(), Ok(()));
    assert!(!claim("C1", 0.31, &[0.1, 0.2]).is_valid());
}

#[test]
fn claim_without_lines() {
    let errs = errors(&claim("C1", 0.0, &[]));
    assert_eq!(
        errs,
        vec![ValidationError::NoServiceLines {
            claim_id: "C1".into()
        }]
    );
}

#[test]
fn claim_empty_id() {
    let errs = errors(&claim("", 10.0, &[10.0]));
    assert_eq!(errs, vec![ValidationError::EmptyClaimId]);
}

#[test]
fn claim_negative_amounts() {
    let errs = errors(&claim("C1", -10.0, &[-10.0]));
    assert!(errs.contains(&ValidationError::NegativeAmount {
        claim_id: "C1".into(),
        field: "total_charge",
        value: -10.0,
    }));
    assert!(errs.contains(&ValidationError::NegativeAmount {
        claim_id: "C1".into(),
        field: "line_charge",
        value: -10.0,
    }));
}

#[test]
fn claim_reports_every_problem_not_just_the_first() {
    let errs = errors(&claim("", -5.0, &[]));
    assert_eq!(errs.len(), 3, "{errs:?}");
    assert!(errs.contains(&ValidationError::EmptyClaimId));
    assert!(errs
        .iter()
        .any(|e| matches!(e, ValidationError::NegativeAmount { .. })));
    assert!(errs
        .iter()
        .any(|e| matches!(e, ValidationError::NoServiceLines { .. })));
}

// ---- Remit ------------------------------------------------------------------

#[test]
fn valid_remit() {
    assert_eq!(
        remit("R1", "1", 150.0, 120.0, &[(100.0, 80.0), (50.0, 40.0)]).validate(),
        Ok(())
    );
}

#[test]
fn denied_remit_is_valid() {
    assert!(remit("R1", "4", 95.0, 0.0, &[(95.0, 0.0)]).is_valid());
}

#[test]
fn remit_without_service_lines_skips_line_checks() {
    assert!(remit("R1", "1", 150.0, 120.0, &[]).is_valid());
}

#[test]
fn remit_unknown_status() {
    let errs = errors(&remit("R1", "99", 10.0, 10.0, &[(10.0, 10.0)]));
    assert_eq!(
        errs,
        vec![ValidationError::UnknownStatus {
            claim_id: "R1".into(),
            code: "99".into()
        }]
    );
}

#[test]
fn remit_payment_mismatch() {
    let errs = errors(&remit(
        "R1",
        "1",
        150.0,
        130.0,
        &[(100.0, 80.0), (50.0, 40.0)],
    ));
    assert_eq!(errs.len(), 1);
    assert!(matches!(
        &errs[0],
        ValidationError::PaymentMismatch { claim_id, declared, computed }
            if claim_id == "R1" && approx(*declared, 130.0) && approx(*computed, 120.0)
    ));
}

#[test]
fn remit_charge_mismatch() {
    let errs = errors(&remit(
        "R1",
        "1",
        999.0,
        120.0,
        &[(100.0, 80.0), (50.0, 40.0)],
    ));
    assert!(
        matches!(&errs[..], [ValidationError::ChargeMismatch { .. }]),
        "{errs:?}"
    );
}

#[test]
fn remit_negative_paid() {
    let errs = errors(&remit("R1", "22", 10.0, -10.0, &[]));
    assert_eq!(
        errs,
        vec![ValidationError::NegativeAmount {
            claim_id: "R1".into(),
            field: "paid",
            value: -10.0
        }]
    );
}

// ---- Remittance / Transaction / Interchange ---------------------------------

fn remittance(total: f64, remits: Vec<Remit<'static>>) -> Remittance<'static> {
    Remittance {
        control_number: "0001",
        total_payment: total,
        remits,
    }
}

#[test]
fn remittance_total_must_match_remits() {
    let ok = remittance(
        150.0,
        vec![
            remit("A", "1", 100.0, 100.0, &[]),
            remit("B", "1", 60.0, 50.0, &[]),
        ],
    );
    assert!(ok.is_valid());

    let bad = remittance(999.0, vec![remit("A", "1", 100.0, 100.0, &[])]);
    let errs = errors(&bad);
    assert!(
        matches!(
            &errs[..],
            [ValidationError::RemittanceTotalMismatch { declared, computed }]
                if approx(*declared, 999.0) && approx(*computed, 100.0)
        ),
        "{errs:?}"
    );
}

#[test]
fn remittance_includes_remit_errors() {
    let r = remittance(
        10.0,
        vec![
            remit("A", "1", 10.0, 10.0, &[]),
            remit("B", "XX", 0.0, 0.0, &[]),
        ],
    );
    let errs = errors(&r);
    assert_eq!(
        errs,
        vec![ValidationError::UnknownStatus {
            claim_id: "B".into(),
            code: "XX".into()
        }]
    );
}

#[test]
fn interchange_collects_errors_from_every_transaction() {
    let ic = Interchange {
        sender_id: "S",
        receiver_id: "R",
        control_number: "1",
        transactions: vec![
            Transaction::Claims(ClaimSet {
                control_number: "0001",
                claims: vec![
                    claim("OK", 10.0, &[10.0]),
                    claim("BAD1", 1.0, &[]),
                    claim("BAD2", 5.0, &[1.0]),
                ],
            }),
            Transaction::Remittance(remittance(0.0, vec![remit("R1", "nope", 0.0, 0.0, &[])])),
        ],
    };
    let errs = errors(&ic);
    assert_eq!(errs.len(), 3, "{errs:?}");
    assert!(matches!(errs[0], ValidationError::NoServiceLines { .. }));
    assert!(matches!(errs[1], ValidationError::ChargeMismatch { .. }));
    assert!(matches!(errs[2], ValidationError::UnknownStatus { .. }));
}

#[test]
fn empty_interchange_is_valid() {
    let ic = Interchange {
        sender_id: "S",
        receiver_id: "R",
        control_number: "1",
        transactions: vec![],
    };
    assert!(ic.is_valid());
}

#[test]
fn validate_all_is_generic() {
    let claims = [
        claim("A", 1.0, &[1.0]),
        claim("B", 2.0, &[]),
        claim("C", 3.0, &[]),
    ];
    assert_eq!(validate_all(&claims).unwrap_err().len(), 2);

    let remits = [remit("A", "1", 1.0, 1.0, &[])];
    assert_eq!(validate_all(&remits), Ok(()));

    let none: [Claim; 0] = [];
    assert_eq!(validate_all(&none), Ok(()));
}

// ---- end to end -------------------------------------------------------------

#[test]
fn every_sample_is_valid() {
    for name in ALL_SAMPLES {
        let input = sample(name);
        let ic = parse_str(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(ic.validate(), Ok(()), "{name}");
    }
}
