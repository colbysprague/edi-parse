//! `build_interchange` (grouping segments into claims/remits) and
//! `to_json` (serde derive).

mod common;

use common::*;
use edi_parse::{
    parse_str, to_json, Adjustment, EdiError, Interchange, Party, Remark, ServiceLine, Transaction,
    Validate,
};
use serde_json::{json, Value};

fn claims<'a>(ic: &'a Interchange<'a>) -> &'a edi_parse::ClaimSet<'a> {
    assert_eq!(ic.transactions.len(), 1);
    match &ic.transactions[0] {
        Transaction::Claims(set) => set,
        other => panic!("expected Claims, got {other:?}"),
    }
}

fn remittance<'a>(ic: &'a Interchange<'a>) -> &'a edi_parse::Remittance<'a> {
    assert_eq!(ic.transactions.len(), 1);
    match &ic.transactions[0] {
        Transaction::Remittance(r) => r,
        other => panic!("expected Remittance, got {other:?}"),
    }
}

fn line(procedure_code: &str, charge: f64, paid: Option<f64>) -> ServiceLine<'_> {
    ServiceLine {
        revenue_code: None,
        procedure_code: Some(procedure_code),
        modifiers: Vec::new(),
        description: None,
        charge,
        paid,
        adjustments: Vec::new(),
        remarks: Vec::new(),
    }
}

// ---- 837P -------------------------------------------------------------------

#[test]
fn interchange_header_fields() {
    let input = sample(S_837_MULTI);
    let ic = parse_str(&input).unwrap();
    assert_eq!(ic.sender_id, "SENDERID");
    assert_eq!(ic.receiver_id, "RECEIVERID");
    assert_eq!(ic.control_number, "000000001");
}

#[test]
fn groups_837_into_claims() {
    let input = sample(S_837_MULTI);
    let ic = parse_str(&input).unwrap();
    let set = claims(&ic);
    assert_eq!(set.control_number, "0001");

    let ids: Vec<&str> = set.claims.iter().map(|c| c.claim_id).collect();
    assert_eq!(ids, ["CLAIM001", "CLAIM002", "CLAIM003"]);

    let totals: Vec<f64> = set.claims.iter().map(|c| c.total_charge).collect();
    assert_eq!(totals, [150.0, 225.5, 80.0]);

    assert_eq!(
        set.claims[0].service_lines,
        vec![line("99213", 100.0, None), line("85025", 50.0, None)]
    );
    assert_eq!(
        set.claims[1].service_lines,
        vec![line("99214", 175.5, None), line("81002", 50.0, None)]
    );
    assert_eq!(set.claims[2].service_lines, vec![line("99213", 80.0, None)]);
}

#[test]
fn each_claim_gets_its_own_subscriber() {
    let input = sample(S_837_MULTI);
    let ic = parse_str(&input).unwrap();
    let names: Vec<(Option<&str>, Option<&str>)> = claims(&ic)
        .claims
        .iter()
        .map(|c| {
            let s = c.subscriber.as_ref().expect("subscriber");
            (s.last_name, s.first_name)
        })
        .collect();
    assert_eq!(
        names,
        [
            (Some("DOE"), Some("JANE")),
            (Some("ROE"), Some("RICHARD")),
            (Some("SMITH"), Some("ALEX"))
        ]
    );
    assert_eq!(
        claims(&ic).claims[1].subscriber.as_ref().unwrap().id,
        Some("MEMBER002")
    );
}

#[test]
fn billing_provider_carries_to_every_claim() {
    let input = sample(S_837_MULTI);
    let ic = parse_str(&input).unwrap();
    let expected = Party {
        last_name: None,
        first_name: None,
        organization_name: Some("EXAMPLE CLINIC"),
        id: Some("1234567893"),
    };
    for claim in &claims(&ic).claims {
        assert_eq!(
            claim.billing_provider.as_ref(),
            Some(&expected),
            "{}",
            claim.claim_id
        );
    }
}

#[test]
fn submitter_and_receiver_nm1_are_not_parties() {
    // NM1*41 (submitter) and NM1*40 (receiver) appear before the billing
    // provider; they must not end up as billing_provider or subscriber.
    let input = sample(S_837_MULTI);
    let ic = parse_str(&input).unwrap();
    let first = &claims(&ic).claims[0];
    assert_ne!(
        first.billing_provider.as_ref().unwrap().organization_name,
        Some("FAKE CLEARINGHOUSE")
    );
    assert_ne!(
        first.subscriber.as_ref().unwrap().last_name,
        Some("FAKE PAYER")
    );
}

#[test]
fn pipe_delimited_837() {
    let input = sample(S_837_PIPE);
    let ic = parse_str(&input).unwrap();
    let set = claims(&ic);
    assert_eq!(set.control_number, "0101");
    assert_eq!(set.claims.len(), 2);
    assert_eq!(set.claims[1].claim_id, "CLAIM102");
    assert_approx(set.claims[1].total_charge, 60.25);
    let codes: Vec<&str> = set.claims[1]
        .service_lines
        .iter()
        .filter_map(|l| l.procedure_code)
        .collect();
    assert_eq!(codes, ["36415", "85025"]);
    assert_eq!(
        set.claims[0]
            .billing_provider
            .as_ref()
            .unwrap()
            .organization_name,
        Some("SAMPLE MEDICAL GROUP")
    );
}

fn patient_last_names<'a>(set: &'a edi_parse::ClaimSet<'a>) -> Vec<Option<&'a str>> {
    set.claims
        .iter()
        .map(|c| c.patient.as_ref().and_then(|p| p.last_name))
        .collect()
}

#[test]
fn patient_in_837_is_kept_on_the_claim() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "NM1*85*2*CLINIC*****XX*1234567893",
            "HL*2*1*22*1",
            "NM1*IL*1*DOE*JOHN****MI*MEMBER001",
            "HL*3*2*23*0",
            "PAT*19",
            "NM1*QC*1*DOE*JANE",
            "CLM*A1*10.00",
            "SV1*HC:99211*10.00",
            "SE*10*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let claim = &claims(&ic).claims[0];
    assert_eq!(
        claim.patient,
        Some(Party {
            last_name: Some("DOE"),
            first_name: Some("JANE"),
            organization_name: None,
            id: None,
        })
    );
    assert_eq!(claim.subscriber.as_ref().unwrap().first_name, Some("JOHN"));

    let v: Value = serde_json::from_str(&to_json(&ic, false).unwrap()).unwrap();
    assert_eq!(
        v["transactions"][0]["claims"][0]["patient"],
        json!({ "last_name": "DOE", "first_name": "JANE" })
    );
}

#[test]
fn claim_without_a_patient_omits_the_key() {
    let v = json_of(S_837_MULTI);
    let claim = v["transactions"][0]["claims"][0].as_object().unwrap();
    assert!(!claim.contains_key("patient"));
}

#[test]
fn patient_applies_to_each_of_their_claims() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "NM1*IL*1*DOE*JOHN",
            "NM1*QC*1*DOE*JANE",
            "CLM*A1*10.00",
            "CLM*A2*20.00",
            "NM1*QC*1*POE*JIM",
            "CLM*A3*30.00",
            "SE*8*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    assert_eq!(
        patient_last_names(claims(&ic)),
        [Some("DOE"), Some("DOE"), Some("POE")]
    );
}

#[test]
fn patient_does_not_carry_to_the_next_subscriber() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "NM1*IL*1*DOE*JOHN",
            "NM1*QC*1*DOE*JANE",
            "CLM*A1*10.00",
            "NM1*IL*1*ROE*RICHARD",
            "CLM*A2*20.00",
            "SE*7*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    assert_eq!(patient_last_names(claims(&ic)), [Some("DOE"), None]);
}

#[test]
fn patient_does_not_carry_to_the_next_transaction() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "NM1*QC*1*DOE*JANE",
            "CLM*A1*10.00",
            "SE*4*0001",
            "ST*837*0002",
            "CLM*B1*20.00",
            "SE*3*0002",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let Transaction::Claims(second) = &ic.transactions[1] else {
        panic!("not claims")
    };
    assert_eq!(second.claims[0].patient, None);
}

// ---- 835 --------------------------------------------------------------------

#[test]
fn patient_before_any_clp_in_835_names_nm1() {
    let input = envelope(
        "HP",
        &["ST*835*0001", "BPR*I*0", "NM1*QC*1*DOE*JANE", "SE*4*0001"],
    );
    match parse_str(&input) {
        Err(EdiError::UnexpectedSegment {
            expected, found, ..
        }) => assert_eq!((expected.as_str(), found.as_str()), ("CLP", "NM1")),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn groups_835_into_remits() {
    let input = sample(S_835_MULTI);
    let ic = parse_str(&input).unwrap();
    let r = remittance(&ic);
    assert_eq!(r.control_number, "0001");
    assert_approx(r.total_payment, 345.0);
    assert_eq!(r.remits.len(), 3);

    let first = &r.remits[0];
    assert_eq!(first.claim_id, "CLAIM001");
    assert_eq!(first.status_code, "1");
    assert_approx(first.total_charge, 150.0);
    assert_approx(first.paid, 120.0);
    assert_eq!(first.patient_responsibility, Some(30.0));
    assert_eq!(
        first.patient,
        Some(Party {
            last_name: Some("DOE"),
            first_name: Some("JANE"),
            organization_name: None,
            id: Some("MEMBER001")
        })
    );
    assert_eq!(
        first.service_lines,
        vec![
            ServiceLine {
                adjustments: vec![adjustment("PR", "1", 15.0), adjustment("PR", "2", 5.0)],
                remarks: vec![Remark {
                    qualifier: "HE",
                    code: "M15",
                }],
                ..line("99213", 100.0, Some(80.0))
            },
            ServiceLine {
                adjustments: vec![adjustment("PR", "2", 10.0)],
                ..line("85025", 50.0, Some(40.0))
            },
        ]
    );

    assert_eq!(r.remits[2].status_code, "2");
    assert_eq!(r.remits[2].service_lines.len(), 1);
}

#[test]
fn crlf_835_with_denial() {
    let input = sample(S_835_CRLF);
    let ic = parse_str(&input).unwrap();
    let r = remittance(&ic);
    assert_approx(r.total_payment, 60.0);
    let denied = &r.remits[0];
    assert_eq!((denied.claim_id, denied.status_code), ("CLAIM201", "4"));
    assert_eq!(denied.paid, 0.0);
    assert_eq!(
        denied.service_lines,
        vec![ServiceLine {
            adjustments: vec![adjustment("CO", "50", 95.0)],
            ..line("97110", 95.0, Some(0.0))
        }]
    );
    assert_eq!(
        r.remits[1].patient.as_ref().unwrap().first_name,
        Some("JANE")
    );
}

#[test]
fn remit_without_patient_or_lines() {
    let input = envelope(
        "HP",
        &[
            "ST*835*0001",
            "BPR*I*10.00",
            "CLP*C1*1*10.00*10.00",
            "SE*4*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let r = remittance(&ic);
    assert_eq!(r.remits.len(), 1);
    assert_eq!(r.remits[0].patient, None);
    assert!(r.remits[0].service_lines.is_empty());
}

// ---- 835 adjustments and remarks ---------------------------------------------

/// Wrap `body` in a single 835 transaction with the right `SE01`.
fn envelope_835(body: &[&str]) -> String {
    let se = format!("SE*{}*0001", body.len() + 2);
    let mut segments = vec!["ST*835*0001"];
    segments.extend_from_slice(body);
    segments.push(&se);
    envelope("HP", &segments)
}

fn adjustment<'a>(group_code: &'a str, reason_code: &'a str, amount: f64) -> Adjustment<'a> {
    Adjustment {
        group_code,
        reason_code,
        amount,
        quantity: None,
    }
}

#[track_caller]
fn assert_orphan(input: &str, expected_position: usize, expected: &str, expected_found: &str) {
    match parse_str(input) {
        Err(EdiError::UnexpectedSegment {
            position,
            expected: exp,
            found,
        }) => {
            assert_eq!(position, expected_position);
            assert_eq!(exp, expected);
            assert_eq!(found, expected_found);
        }
        other => panic!("expected UnexpectedSegment, got {other:?}"),
    }
}

#[test]
fn cas_after_clp_is_a_claim_level_adjustment() {
    let input = envelope_835(&[
        "CLP*C1*1*100.00*70.00",
        "CAS*CO*45*20.00",
        "NM1*QC*1*DOE*JANE",
        "CAS*PR*1*10.00",
        "SVC*HC:99213*100.00*70.00",
    ]);
    let ic = parse_str(&input).unwrap();
    let remit = &remittance(&ic).remits[0];
    assert_eq!(
        remit.adjustments,
        [adjustment("CO", "45", 20.0), adjustment("PR", "1", 10.0)]
    );
    assert!(remit.service_lines[0].adjustments.is_empty());
}

#[test]
fn cas_after_svc_attaches_to_that_line() {
    let input = envelope_835(&[
        "CLP*C1*1*150.00*120.00",
        "SVC*HC:99213*100.00*80.00",
        "CAS*PR*2*20.00",
        "SVC*HC:85025*50.00*40.00",
        "CAS*CO*45*10.00",
    ]);
    let ic = parse_str(&input).unwrap();
    let remit = &remittance(&ic).remits[0];
    assert!(remit.adjustments.is_empty());
    assert_eq!(
        remit.service_lines[0].adjustments,
        [adjustment("PR", "2", 20.0)]
    );
    assert_eq!(
        remit.service_lines[1].adjustments,
        [adjustment("CO", "45", 10.0)]
    );
}

#[test]
fn multiple_cas_on_a_line_flatten_in_order() {
    let input = envelope_835(&[
        "CLP*C1*1*100.00*60.00",
        "SVC*HC:99213*100.00*60.00",
        "CAS*CO*45*10.00*2*94*5.00",
        "CAS*PR*1*25.00",
    ]);
    let ic = parse_str(&input).unwrap();
    assert_eq!(
        remittance(&ic).remits[0].service_lines[0].adjustments,
        [
            Adjustment {
                quantity: Some(2.0),
                ..adjustment("CO", "45", 10.0)
            },
            adjustment("CO", "94", 5.0),
            adjustment("PR", "1", 25.0),
        ]
    );
}

#[test]
fn cas_after_a_later_clp_is_on_the_new_remit() {
    let input = envelope_835(&[
        "CLP*C1*1*10.00*10.00",
        "SVC*HC:99213*10.00*10.00",
        "CLP*C2*1*20.00*15.00",
        "CAS*CO*45*5.00",
    ]);
    let ic = parse_str(&input).unwrap();
    let remits = &remittance(&ic).remits;
    assert!(remits[0].service_lines[0].adjustments.is_empty());
    assert_eq!(remits[1].adjustments, [adjustment("CO", "45", 5.0)]);
}

#[test]
fn lq_after_svc_is_a_remark_on_that_line() {
    let input = envelope_835(&[
        "CLP*C1*1*150.00*120.00",
        "SVC*HC:99213*100.00*80.00",
        "CAS*PR*2*20.00",
        "LQ*HE*N130",
        "LQ*RX*70",
        "SVC*HC:85025*50.00*40.00",
    ]);
    let ic = parse_str(&input).unwrap();
    let lines = &remittance(&ic).remits[0].service_lines;
    assert_eq!(
        lines[0].remarks,
        [
            Remark {
                qualifier: "HE",
                code: "N130"
            },
            Remark {
                qualifier: "RX",
                code: "70"
            },
        ]
    );
    assert!(lines[1].remarks.is_empty());
}

#[test]
fn cas_before_any_clp_is_an_error() {
    // position: ISA, GS, ST, CAS
    let input = envelope_835(&["BPR*I*0", "CAS*CO*45*5.00"]);
    assert_orphan(&input, 4, "CLP", "CAS");
}

#[test]
fn lq_before_any_clp_is_an_error() {
    let input = envelope_835(&["LQ*HE*N130"]);
    assert_orphan(&input, 3, "CLP", "LQ");
}

#[test]
fn lq_after_clp_is_a_claim_level_remark() {
    let input = envelope_835(&[
        "CLP*C1*1*10.00*10.00",
        "LQ*HE*N25",
        "SVC*HC:99213*10.00*10.00",
    ]);
    let ic = parse_str(&input).unwrap();
    let remit = &remittance(&ic).remits[0];
    assert_eq!(
        remit.remarks,
        [Remark {
            qualifier: "HE",
            code: "N25"
        }]
    );
    assert!(remit.service_lines[0].remarks.is_empty());
}

#[test]
fn lq_after_a_later_clp_does_not_reach_the_previous_line() {
    let input = envelope_835(&[
        "CLP*C1*1*10.00*10.00",
        "SVC*HC:99213*10.00*10.00",
        "CLP*C2*1*20.00*20.00",
        "LQ*HE*N130",
    ]);
    let ic = parse_str(&input).unwrap();
    let remits = &remittance(&ic).remits;
    assert!(remits[0].service_lines[0].remarks.is_empty());
    assert_eq!(remits[1].remarks.len(), 1);
}

#[test]
fn every_x12_example_parses() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/x12");
    let files = edi_parse::collect_files_recursive(&root).unwrap();
    assert!(!files.is_empty());
    for file in files {
        let input = std::fs::read_to_string(&file).unwrap();
        parse_str(&input).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    }
}

#[test]
fn cas_after_se_is_an_error() {
    let input = envelope(
        "HP",
        &[
            "ST*835*0001",
            "CLP*C1*1*10.00*10.00",
            "SVC*HC:99213*10.00*10.00",
            "SE*4*0001",
            "CAS*CO*45*5.00",
        ],
    );
    assert_orphan(&input, 6, "CLP", "CAS");
}

#[test]
fn cas_does_not_reach_the_previous_transaction() {
    let input = envelope(
        "HP",
        &[
            "ST*835*0001",
            "CLP*C1*1*10.00*10.00",
            "SE*3*0001",
            "ST*835*0002",
            "CAS*CO*45*5.00",
            "SE*3*0002",
        ],
    );
    assert_orphan(&input, 6, "CLP", "CAS");
}

#[test]
fn cas_and_lq_in_an_837_are_ignored() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "CAS*CO*45*5.00",
            "CLM*C1*10.00",
            "CAS*CO*45*5.00",
            "LQ*UT*04.03",
            "SV1*HC:99213*10.00",
            "LQ*UT*04.03",
            "CAS*PR*1*5.00",
            "SE*9*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    assert_eq!(
        claims(&ic).claims[0].service_lines,
        [line("99213", 10.0, None)]
    );
}

#[test]
fn sample_835_has_adjustments_at_both_levels() {
    let input = sample(S_835_MULTI);
    let ic = parse_str(&input).unwrap();
    let remits = &remittance(&ic).remits;

    assert_eq!(remits[0].adjustments, []);
    assert_eq!(
        remits[0].service_lines[0].adjustments,
        [adjustment("PR", "1", 15.0), adjustment("PR", "2", 5.0)]
    );
    assert_eq!(remits[0].service_lines[0].remarks[0].code, "M15");
    assert_eq!(remits[2].adjustments, [adjustment("PR", "1", 25.0)]);
    assert_eq!(
        remits[2].service_lines[0].adjustments,
        [adjustment("PR", "2", 10.0)]
    );
    assert_eq!(remits[2].service_lines[0].remarks[0].code, "N130");
}

// ---- structure --------------------------------------------------------------

#[test]
fn multiple_transactions_in_one_interchange() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "NM1*85*2*CLINIC A*****XX*1234567893",
            "CLM*A1*10.00",
            "SV1*HC:99211*10.00",
            "SE*5*0001",
            "ST*837*0002",
            "NM1*85*2*CLINIC B*****XX*1234567893",
            "CLM*B1*20.00",
            "SV1*HC:99212*20.00",
            "SE*5*0002",
        ],
    );
    let ic = parse_str(&input).unwrap();
    assert_eq!(ic.transactions.len(), 2);
    let Transaction::Claims(second) = &ic.transactions[1] else {
        panic!("not claims")
    };
    assert_eq!(second.control_number, "0002");
    assert_eq!(second.claims[0].claim_id, "B1");
    assert_eq!(
        second.claims[0]
            .billing_provider
            .as_ref()
            .unwrap()
            .organization_name,
        Some("CLINIC B")
    );
}

#[test]
fn nm1_with_blank_name_does_not_fail_the_file() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "NM1*87*2",
            "NM1*85*2******XX*1234567893",
            "CLM*A1*10.00",
            "SV1*HC:99211*10.00",
            "SE*6*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let provider = claims(&ic).claims[0].billing_provider.as_ref().unwrap();
    assert_eq!(provider.organization_name, None);
    assert_eq!(provider.id, Some("1234567893"));
    assert_eq!(
        serde_json::to_value(provider).unwrap(),
        json!({ "id": "1234567893" })
    );
}

fn with_billing_provider(nm1: &str) -> String {
    envelope(
        "HC",
        &[
            "ST*837*0001",
            nm1,
            "CLM*A1*10.00",
            "SV1*HC:99211*10.00",
            "SE*5*0001",
        ],
    )
}

#[test]
fn nm102_of_1_is_a_person() {
    let input = with_billing_provider("NM1*85*1*JONES*ANN****XX*1234567893");
    let ic = parse_str(&input).unwrap();
    assert_eq!(
        claims(&ic).claims[0].billing_provider,
        Some(Party {
            last_name: Some("JONES"),
            first_name: Some("ANN"),
            organization_name: None,
            id: Some("1234567893"),
        })
    );
}

#[test]
fn nm102_of_2_is_an_organization() {
    let input = with_billing_provider("NM1*85*2*CLINIC*****XX*1234567893");
    let ic = parse_str(&input).unwrap();
    assert_eq!(
        claims(&ic).claims[0].billing_provider,
        Some(Party {
            last_name: None,
            first_name: None,
            organization_name: Some("CLINIC"),
            id: Some("1234567893"),
        })
    );
}

#[test]
fn invalid_nm102_on_a_kept_nm1_is_an_error() {
    let input = with_billing_provider("NM1*85*9*CLINIC*****XX*1234567893");
    assert!(matches!(
        parse_str(&input),
        Err(EdiError::InvalidEntityType { value }) if value == "9"
    ));
}

#[test]
fn invalid_nm102_on_an_ignored_nm1_is_not_an_error() {
    let input = with_billing_provider("NM1*87*9");
    assert!(parse_str(&input).is_ok());
}

#[test]
fn transaction_with_no_claims() {
    let input = envelope("HC", &["ST*837*0001", "BHT*0019", "SE*3*0001"]);
    let ic = parse_str(&input).unwrap();
    assert!(claims(&ic).claims.is_empty());
}

#[test]
fn unsupported_transaction_set() {
    let input = envelope("BE", &["ST*834*0001", "SE*2*0001"]);
    match parse_str(&input) {
        Err(EdiError::UnsupportedTransaction(id)) => assert_eq!(id, "834"),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn institutional_claim_keeps_its_lines() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "CLM*A1*30.00",
            "SV2*0305*HC:85025:QW*10.00*UN*1",
            "SV2*0120**20.00*DA*1",
            "SE*5*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let lines = &claims(&ic).claims[0].service_lines;
    assert_eq!(
        lines[0],
        ServiceLine {
            revenue_code: Some("0305"),
            modifiers: vec!["QW"],
            ..line("85025", 10.0, None)
        }
    );
    assert_eq!(lines[1].revenue_code, Some("0120"));
    assert_eq!(lines[1].procedure_code, None);
    assert_eq!(lines[1].charge, 20.0);

    let v: Value = serde_json::from_str(&to_json(&ic, false).unwrap()).unwrap();
    assert_eq!(
        v["transactions"][0]["claims"][0]["service_lines"][1],
        json!({ "revenue_code": "0120", "charge": 20.0 })
    );
    assert!(claims(&ic).claims[0].is_valid());
}

#[test]
fn dental_claim_keeps_its_lines() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "CLM*A1*50.00",
            "SV3*AD:D2150*50.00****1",
            "SE*4*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let claim = &claims(&ic).claims[0];
    assert_eq!(claim.service_lines, vec![line("D2150", 50.0, None)]);
    assert!(claim.is_valid());
}

#[test]
fn institutional_and_dental_lines_need_an_open_claim() {
    for (segment, id) in [
        ("SV2*0305*HC:85025*10.00", "SV2"),
        ("SV3*AD:D1110*10.00", "SV3"),
    ] {
        let input = envelope("HC", &["ST*837*0001", segment, "SE*3*0001"]);
        assert!(matches!(
            parse_str(&input),
            Err(EdiError::UnexpectedSegment { ref found, .. }) if found == id
        ));

        let input = envelope(
            "HP",
            &["ST*835*0001", "CLP*C1*1*10.00*10.00", segment, "SE*4*0001"],
        );
        assert!(matches!(
            parse_str(&input),
            Err(EdiError::UnexpectedSegment { ref found, .. }) if found == id
        ));
    }
}

#[test]
fn service_line_before_any_claim() {
    let input = envelope("HC", &["ST*837*0001", "SV1*HC:99213*10.00", "SE*3*0001"]);
    assert!(matches!(
        parse_str(&input),
        Err(EdiError::UnexpectedSegment { ref found, .. }) if found == "SV1"
    ));
    let input = envelope(
        "HP",
        &[
            "ST*835*0001",
            "BPR*I*0",
            "SVC*HC:99213*10.00*0",
            "SE*4*0001",
        ],
    );
    assert!(matches!(
        parse_str(&input),
        Err(EdiError::UnexpectedSegment { ref found, .. }) if found == "SVC"
    ));
}

#[test]
fn service_lines_keep_modifiers_and_description() {
    let input = envelope(
        "HC",
        &[
            "ST*837*0001",
            "CLM*A1*20.00",
            "SV1*HC:K0001:RR:KH:BR*10.00",
            "SV1*HC:J3490:::::Hep Lock see NDC#*10.00",
            "SE*5*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let v: Value = serde_json::from_str(&to_json(&ic, false).unwrap()).unwrap();
    let lines = &v["transactions"][0]["claims"][0]["service_lines"];
    assert_eq!(
        lines[0],
        json!({ "procedure_code": "K0001", "modifiers": ["RR", "KH", "BR"], "charge": 10.0 })
    );
    assert_eq!(
        lines[1],
        json!({ "procedure_code": "J3490", "description": "Hep Lock see NDC#", "charge": 10.0 })
    );
}

#[test]
fn remit_lines_keep_modifiers() {
    let input = envelope(
        "HP",
        &[
            "ST*835*0001",
            "BPR*I*8.00",
            "CLP*C1*1*10.00*8.00",
            "SVC*HC:99213:25*10.00*8.00",
            "SE*5*0001",
        ],
    );
    let ic = parse_str(&input).unwrap();
    let line = &remittance(&ic).remits[0].service_lines[0];
    assert_eq!(line.procedure_code, Some("99213"));
    assert_eq!(line.modifiers, ["25"]);
}

#[test]
fn component_separator_does_not_change_the_result() {
    for sep in [':', '>', '|'] {
        let mut input = isa('*', '^', sep, '~');
        input.push_str("GS*HC*S*R*20261017*1200*1*X*005010~ST*837*0001~CLM*A1*10.00~");
        input.push_str(&format!("SV1*HC{sep}K0001{sep}RR{sep}KH*10.00~"));
        input.push_str("SE*4*0001~GE*1*1~IEA*1*000000001~");
        let ic = parse_str(&input).unwrap();
        let line = &claims(&ic).claims[0].service_lines[0];
        assert_eq!(line.procedure_code, Some("K0001"), "{sep}");
        assert_eq!(line.modifiers, ["RR", "KH"], "{sep}");
    }
}

#[test]
fn parsed_data_borrows_from_input() {
    let input = sample(S_837_MULTI);
    let range = input.as_bytes().as_ptr_range();
    let ic = parse_str(&input).unwrap();
    for claim in &claims(&ic).claims {
        assert!(
            range.contains(&claim.claim_id.as_ptr()),
            "claim_id was copied"
        );
        for l in &claim.service_lines {
            assert!(
                range.contains(&l.procedure_code.unwrap().as_ptr()),
                "procedure_code was copied"
            );
            assert!(
                l.modifiers.iter().all(|m| range.contains(&m.as_ptr())),
                "modifiers were copied"
            );
        }
    }
}

// ---- JSON (serde) -----------------------------------------------------------

fn json_of(name: &str) -> Value {
    let input = sample(name);
    let ic = parse_str(&input).unwrap();
    serde_json::from_str(&to_json(&ic, false).unwrap()).expect("to_json must produce valid JSON")
}

#[test]
fn json_837_shape() {
    let v = json_of(S_837_MULTI);
    assert_eq!(v["sender_id"], "SENDERID");
    assert_eq!(v["control_number"], "000000001");

    let t = &v["transactions"][0];
    assert_eq!(
        t["type"], "claims",
        "Transaction should be internally tagged with \"type\""
    );
    assert_eq!(t["control_number"], "0001");

    let claim = &t["claims"][0];
    assert_eq!(claim["claim_id"], "CLAIM001");
    assert_eq!(claim["total_charge"], 150.0);
    assert_eq!(
        claim["billing_provider"],
        json!({ "organization_name": "EXAMPLE CLINIC", "id": "1234567893" }),
        "None fields (first_name) should be omitted, not null"
    );
    assert_eq!(
        claim["service_lines"][0],
        json!({ "procedure_code": "99213", "charge": 100.0 }),
        "837 lines have no `paid` key"
    );
}

#[test]
fn json_835_shape() {
    let v = json_of(S_835_MULTI);
    let t = &v["transactions"][0];
    assert_eq!(t["type"], "remittance");
    assert_eq!(t["total_payment"], 345.0);
    let remit = &t["remits"][0];
    assert_eq!(remit["status_code"], "1");
    assert_eq!(remit["patient_responsibility"], 30.0);
    assert_eq!(remit["patient"]["first_name"], "JANE");
    assert_eq!(
        remit["service_lines"][1],
        json!({
            "procedure_code": "85025",
            "charge": 50.0,
            "paid": 40.0,
            "adjustments": [{ "group_code": "PR", "reason_code": "2", "amount": 10.0 }],
        })
    );
}

#[test]
fn json_835_adjustments_and_remarks() {
    let v = json_of(S_835_MULTI);
    let remits = &v["transactions"][0]["remits"];

    assert!(remits[0].get("adjustments").is_none());
    assert_eq!(
        remits[0]["service_lines"][0]["adjustments"],
        json!([
            { "group_code": "PR", "reason_code": "1", "amount": 15.0 },
            { "group_code": "PR", "reason_code": "2", "amount": 5.0 },
        ])
    );
    assert_eq!(
        remits[0]["service_lines"][0]["remarks"],
        json!([{ "qualifier": "HE", "code": "M15" }])
    );
    assert_eq!(
        remits[2]["adjustments"],
        json!([{ "group_code": "PR", "reason_code": "1", "amount": 25.0 }])
    );
}

#[test]
fn json_837_lines_have_no_adjustment_keys() {
    let v = json_of(S_837_MULTI);
    let line = &v["transactions"][0]["claims"][0]["service_lines"][0];
    assert!(line.get("adjustments").is_none());
    assert!(line.get("remarks").is_none());
}

#[test]
fn json_has_no_nulls_anywhere() {
    fn walk(v: &Value, path: &str) {
        match v {
            Value::Null => panic!("null at {path}"),
            Value::Array(items) => items
                .iter()
                .enumerate()
                .for_each(|(i, x)| walk(x, &format!("{path}[{i}]"))),
            Value::Object(map) => map
                .iter()
                .for_each(|(k, x)| walk(x, &format!("{path}.{k}"))),
            _ => {}
        }
    }
    for name in ALL_SAMPLES {
        walk(&json_of(name), name);
    }
}

#[test]
fn json_pretty_flag() {
    let input = sample(S_835_CRLF);
    let ic = parse_str(&input).unwrap();
    let compact = to_json(&ic, false).unwrap();
    let pretty = to_json(&ic, true).unwrap();
    assert!(!compact.contains('\n'));
    assert!(pretty.contains('\n'));
    let a: Value = serde_json::from_str(&compact).unwrap();
    let b: Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(a, b);
}
