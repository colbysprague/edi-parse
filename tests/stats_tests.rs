//! `Summary` built with iterator chains.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::*;
use edi_parse::{
    parse_str, Claim, ClaimSet, EdiError, Interchange, ServiceLine, Summary, Transaction,
};

fn summarize(name: &str) -> Summary {
    let input = sample(name);
    let ic = parse_str(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
    Summary::from_interchange(&ic)
}

fn in_file(path: &str, source: EdiError) -> EdiError {
    EdiError::InFile {
        path: PathBuf::from(path),
        source: Box::new(source),
    }
}

fn counts(pairs: &[(&str, usize)]) -> BTreeMap<String, usize> {
    pairs.iter().map(|&(k, v)| (k.to_string(), v)).collect()
}

#[test]
fn summary_of_837() {
    let s = summarize(S_837_MULTI);
    assert_eq!(s.files_processed, 1);
    assert_eq!(s.files_failed, 0);
    assert!(s.failed_files.is_empty());
    assert_eq!(s.transactions, 1);
    assert_eq!(s.claims, 3);
    assert_eq!(s.remits, 0);
    assert_eq!(s.service_lines, 5);
    assert_approx(s.total_billed, 455.5);
    assert_approx(s.total_paid, 0.0);
    assert_eq!(s.validation_errors, 0);
    assert_eq!(
        s.procedure_counts,
        counts(&[("99213", 2), ("85025", 1), ("99214", 1), ("81002", 1)])
    );
}

#[test]
fn summary_of_835() {
    let s = summarize(S_835_MULTI);
    assert_eq!(s.claims, 0);
    assert_eq!(s.remits, 3);
    assert_eq!(s.service_lines, 5);
    assert_approx(s.total_billed, 0.0);
    assert_approx(s.total_paid, 345.0);
}

#[test]
fn procedure_counts_ignore_modifiers_and_descriptions() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "CLM*A1*40.00",
            "SV1*HC:98940*10.00",
            "SV1*HC:98940:AT*10.00",
            "SV1*HC:J3490:::::Hep Lock see NDC#*10.00",
            "SV1*HC:J3490:::::Sod Chl 0.9% see NDC#*10.00",
            "SE*7*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    assert_eq!(
        Summary::from_interchange(&ic).procedure_counts,
        counts(&[("98940", 2), ("J3490", 2)])
    );
}

#[test]
fn summary_includes_institutional_and_dental_lines() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "CLM*A1*30.00",
            "SV2*0305*HC:85025*10.00*UN*1",
            "SV2*0120**20.00*DA*1",
            "CLM*B1*40.00",
            "SV3*AD:D1110*40.00****1",
            "SE*7*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let summary = Summary::from_interchange(&ic);
    assert_eq!(summary.service_lines, 3);
    assert_eq!(summary.validation_errors, 0);
    assert_eq!(
        summary.procedure_counts,
        counts(&[("85025", 1), ("D1110", 1)])
    );
}

#[test]
fn summary_counts_validation_errors() {
    let bad = |id| Claim {
        claim_id: id,
        total_charge: 10.0,
        billing_provider: None,
        subscriber: None,
        patient: None,
        service_lines: vec![ServiceLine {
            revenue_code: None,
            procedure_code: Some("99213"),
            modifiers: Vec::new(),
            description: None,
            charge: 1.0,
            paid: None,
            adjustments: Vec::new(),
            remarks: Vec::new(),
        }],
    };
    let ic = Interchange {
        sender_id: "S",
        receiver_id: "R",
        control_number: "1",
        transactions: vec![Transaction::Claims(ClaimSet {
            control_number: "0001",
            claims: vec![bad("A"), bad("B")],
        })],
    };
    let s = Summary::from_interchange(&ic);
    assert_eq!(s.validation_errors, 2);
    assert_eq!(s.claims, 2);
    assert_approx(s.total_billed, 20.0);
}

#[test]
fn failure_summary() {
    let err = in_file("data/invalid/missing_iea.edi", EdiError::MissingIea);
    let s = Summary::failure(&err);
    assert_eq!(s.files_processed, 0);
    assert_eq!(s.files_failed, 1);
    assert_eq!(s.failed_files.len(), 1);
    assert!(
        s.failed_files[0].contains("missing_iea.edi"),
        "{:?}",
        s.failed_files
    );
    assert!(
        s.failed_files[0].contains(&err.to_string()),
        "{:?}",
        s.failed_files
    );
    assert_eq!(s.claims, 0);
}

#[test]
fn merge_adds_everything() {
    let a = summarize(S_837_MULTI);
    let b = summarize(S_835_MULTI);
    let m = a.merge(b);
    assert_eq!(m.files_processed, 2);
    assert_eq!(m.transactions, 2);
    assert_eq!(m.claims, 3);
    assert_eq!(m.remits, 3);
    assert_eq!(m.service_lines, 10);
    assert_approx(m.total_billed, 455.5);
    assert_approx(m.total_paid, 345.0);
    assert_eq!(m.procedure_counts["99213"], 4);
    assert_eq!(m.procedure_counts["85025"], 2);
}

#[test]
fn from_interchange_breaks_down_validation_errors() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "CLM*A1*20.00",
            "SV1*HC:99213*10.00",
            "CLM*B1*20.00",
            "SV1*HC:99213*10.00",
            "CLM*C1*20.00",
            "SE*7*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let s = Summary::from_interchange(&ic);
    assert_eq!(s.validation_errors, 3);
    assert_eq!(
        s.validation_error_counts,
        counts(&[("charge_mismatch", 2), ("no_service_lines", 1)])
    );
    assert_eq!(s.files_with_validation_errors, 1);
}

#[test]
fn valid_file_has_no_validation_error_counts() {
    let s = summarize(S_837_MULTI);
    assert!(s.validation_error_counts.is_empty());
    assert_eq!(s.files_with_validation_errors, 0);
}

#[test]
fn merge_adds_validation_error_counts() {
    let a = Summary {
        validation_errors: 3,
        validation_error_counts: counts(&[("charge_mismatch", 2), ("no_service_lines", 1)]),
        files_with_validation_errors: 1,
        ..Summary::default()
    };
    let b = Summary {
        validation_errors: 2,
        validation_error_counts: counts(&[("charge_mismatch", 1), ("unknown_status", 1)]),
        files_with_validation_errors: 1,
        ..Summary::default()
    };

    let m = a.merge(b);
    assert_eq!(m.validation_errors, 5);
    assert_eq!(
        m.validation_error_counts,
        counts(&[
            ("charge_mismatch", 3),
            ("no_service_lines", 1),
            ("unknown_status", 1)
        ])
    );
    assert_eq!(m.files_with_validation_errors, 2);
    assert_eq!(m.clone().merge(Summary::default()), m);
}

#[test]
fn merge_keeps_failed_files_sorted() {
    let z = Summary::failure(&in_file("z.edi", EdiError::MissingIea));
    let a = Summary::failure(&in_file("a.edi", EdiError::MissingIsa));
    let m = z.merge(a);
    assert_eq!(m.files_failed, 2);
    assert!(
        m.failed_files[0].starts_with("a.edi"),
        "{:?}",
        m.failed_files
    );
    assert!(
        m.failed_files[1].starts_with("z.edi"),
        "{:?}",
        m.failed_files
    );
}

#[test]
fn default_is_merge_identity() {
    let s = summarize(S_835_CRLF);
    assert_eq!(s.clone().merge(Summary::default()), s);
    assert_eq!(Summary::default().merge(s.clone()), s);
}

#[test]
fn merge_is_order_independent_for_counts() {
    let names = ALL_SAMPLES;
    let forward = names
        .iter()
        .map(|n| summarize(n))
        .fold(Summary::default(), Summary::merge);
    let backward = names
        .iter()
        .rev()
        .map(|n| summarize(n))
        .fold(Summary::default(), Summary::merge);
    assert_eq!(forward.procedure_counts, backward.procedure_counts);
    assert_eq!(forward.service_lines, backward.service_lines);
    assert_approx(forward.total_billed, backward.total_billed);
}

#[test]
fn all_samples_merged() {
    let s = ALL_SAMPLES
        .iter()
        .map(|n| summarize(n))
        .fold(Summary::default(), Summary::merge);
    assert_eq!(s.files_processed, 4);
    assert_eq!(s.transactions, 4);
    assert_eq!(s.claims, 5);
    assert_eq!(s.remits, 5);
    assert_eq!(s.service_lines, 16);
    assert_approx(s.total_billed, 635.75);
    assert_approx(s.total_paid, 405.0);
    assert_eq!(s.validation_errors, 0);
    assert_eq!(
        s.procedure_counts,
        counts(&[
            ("99213", 5),
            ("85025", 4),
            ("99214", 2),
            ("81002", 2),
            ("36415", 2),
            ("97110", 1),
        ])
    );
}

#[test]
fn top_procedures_orders_by_count_then_code() {
    let s = ALL_SAMPLES
        .iter()
        .map(|n| summarize(n))
        .fold(Summary::default(), Summary::merge);
    assert_eq!(s.top_procedures(1), vec![("99213", 5)]);
    assert_eq!(
        s.top_procedures(4),
        vec![("99213", 5), ("85025", 4), ("36415", 2), ("81002", 2)]
    );
    assert_eq!(s.top_procedures(100).len(), 6);
    assert!(s.top_procedures(0).is_empty());
}

#[test]
fn top_procedures_of_empty_summary() {
    assert!(Summary::default().top_procedures(5).is_empty());
}

#[test]
fn summary_serializes_to_json() {
    let v = serde_json::to_value(summarize(S_837_PIPE)).unwrap();
    assert_eq!(v["claims"], 2);
    assert_eq!(v["procedure_counts"]["36415"], 1);
}

#[test]
fn totals_serialize_in_whole_cents() {
    let summary = Summary {
        total_billed: 239212541.76999974,
        total_paid: 158766257.14999998,
        ..Summary::default()
    };
    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["total_billed"], 239212541.77);
    assert_eq!(json["total_paid"], 158766257.15);
}

#[test]
fn totals_serialize_the_same_in_any_merge_order() {
    let amounts = [0.1, 0.2, 0.3, 1234567.89, 0.07];
    let one = |total_billed| Summary {
        total_billed,
        ..Summary::default()
    };
    let forward = amounts
        .iter()
        .fold(Summary::default(), |t, &a| t.merge(one(a)));
    let backward = amounts
        .iter()
        .rev()
        .fold(Summary::default(), |t, &a| t.merge(one(a)));

    assert_eq!(
        serde_json::to_string(&forward).unwrap(),
        serde_json::to_string(&backward).unwrap(),
    );
}
