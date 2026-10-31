//! Task 5: `build_interchange` (grouping segments into claims/remits) and
//! `to_json` (serde derive).

mod common;

use common::*;
use edi_parse::{parse_str, to_json, EdiError, Interchange, Party, ServiceLine, Transaction};
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
        procedure_code,
        charge,
        paid,
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
        .map(|l| l.procedure_code)
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

// ---- 835 --------------------------------------------------------------------

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
            line("99213", 100.0, Some(80.0)),
            line("85025", 50.0, Some(40.0))
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
    assert_eq!(denied.service_lines, vec![line("97110", 95.0, Some(0.0))]);
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
                range.contains(&l.procedure_code.as_ptr()),
                "procedure_code was copied"
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
        json!({ "procedure_code": "85025", "charge": 50.0, "paid": 40.0 })
    );
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
