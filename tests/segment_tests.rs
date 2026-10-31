//! Task 3: `Segment::from_raw`, `Segment::id`, and the parsing helpers.
//!
//! Raw segments are built by the test helper `raw` below (not your tokenizer),
//! so these tests only depend on Task 1's `RawSegment::element`.

mod common;

use common::*;
use edi_parse::segment::*;
use edi_parse::{Delimiters, EdiError, RawSegment, Segment};

const STD: Delimiters = Delimiters::STANDARD;

/// Split a standard-delimiter segment (no `~`) into a RawSegment.
fn raw(s: &str) -> RawSegment<'_> {
    let mut parts = s.split('*');
    RawSegment {
        id: parts.next().unwrap(),
        elements: parts.collect(),
    }
}

fn parse(s: &str) -> Segment<'_> {
    Segment::from_raw(raw(s), &STD).unwrap_or_else(|e| panic!("{s:?}: {e}"))
}

fn parse_err(s: &str) -> EdiError {
    match Segment::from_raw(raw(s), &STD) {
        Ok(seg) => panic!("{s:?} should fail, got {seg:?}"),
        Err(e) => e,
    }
}

#[test]
fn isa_trims_padding() {
    let input = isa('*', '^', ':', '~');
    let seg = parse(input.trim_end_matches('~'));
    assert_eq!(
        seg,
        Segment::Isa(Isa {
            sender_id: "SENDERID",
            receiver_id: "RECEIVERID",
            date: "261017",
            time: "1200",
            control_number: "000000001",
            usage: "T",
        })
    );
}

#[test]
fn gs() {
    assert_eq!(
        parse("GS*HC*SENDERID*RECEIVERID*20261017*1200*1*X*005010X222A1"),
        Segment::Gs(Gs {
            functional_id: "HC",
            sender: "SENDERID",
            receiver: "RECEIVERID",
            control_number: "1",
            version: "005010X222A1",
        })
    );
}

#[test]
fn st_with_and_without_implementation_reference() {
    let expected = Segment::St(St {
        transaction_set_id: "837",
        control_number: "0001",
    });
    assert_eq!(parse("ST*837*0001*005010X222A1"), expected);
    assert_eq!(parse("ST*837*0001"), expected);
}

#[test]
fn bpr() {
    assert_eq!(
        parse("BPR*I*345.00*C*ACH*CCP*01*999999999*DA*123456"),
        Segment::Bpr(Bpr {
            handling_code: "I",
            total_payment: 345.0
        })
    );
}

#[test]
fn nm1_person() {
    assert_eq!(
        parse("NM1*IL*1*DOE*JANE****MI*MEMBER001"),
        Segment::Nm1(Nm1 {
            entity_id: "IL",
            entity_type: "1",
            last_name: Some("DOE"),
            first_name: Some("JANE"),
            id_qualifier: Some("MI"),
            id: Some("MEMBER001"),
        })
    );
}

#[test]
fn nm1_organization_has_no_first_name() {
    assert_eq!(
        parse("NM1*85*2*EXAMPLE CLINIC*****XX*1234567893"),
        Segment::Nm1(Nm1 {
            entity_id: "85",
            entity_type: "2",
            last_name: Some("EXAMPLE CLINIC"),
            first_name: None,
            id_qualifier: Some("XX"),
            id: Some("1234567893"),
        })
    );
}

#[test]
fn nm1_without_id() {
    let Segment::Nm1(nm1) = parse("NM1*QC*1*DOE*JANE") else {
        panic!("not NM1")
    };
    assert_eq!((nm1.id_qualifier, nm1.id), (None, None));
}

#[test]
fn nm1_without_name() {
    assert_eq!(
        parse("NM1*87*2"),
        Segment::Nm1(Nm1 {
            entity_id: "87",
            entity_type: "2",
            last_name: None,
            first_name: None,
            id_qualifier: None,
            id: None,
        })
    );
}

#[test]
fn clm() {
    assert_eq!(
        parse("CLM*CLAIM001*150.00***11:B:1*Y*A*Y*Y"),
        Segment::Clm(Clm {
            claim_id: "CLAIM001",
            total_charge: 150.0
        })
    );
}

#[test]
fn sv1_splits_composite_procedure() {
    assert_eq!(
        parse("SV1*HC:99213*100.00*UN*1***1"),
        Segment::Sv1(Sv1 {
            qualifier: Some("HC"),
            procedure_code: "99213",
            charge: 100.0,
            units: Some(1.0),
        })
    );
}

#[test]
fn sv1_uses_the_component_separator_from_delimiters() {
    let pipes = Delimiters {
        element: '|',
        repetition: '{',
        component: '>',
        segment: '^',
    };
    let raw = RawSegment {
        id: "SV1",
        elements: vec!["HC>36415", "10.25", "UN", "2"],
    };
    let Segment::Sv1(sv1) = Segment::from_raw(raw, &pipes).unwrap() else {
        panic!("not SV1")
    };
    assert_eq!(sv1.qualifier, Some("HC"));
    assert_eq!(sv1.procedure_code, "36415");
    assert_eq!(sv1.units, Some(2.0));
}

#[test]
fn sv1_without_component_separator_or_units() {
    let Segment::Sv1(sv1) = parse("SV1*99213*100.00") else {
        panic!("not SV1")
    };
    assert_eq!(sv1.qualifier, None);
    assert_eq!(sv1.procedure_code, "99213");
    assert_eq!(sv1.units, None);
}

#[test]
fn sv1_bad_units_is_an_error_not_none() {
    match parse_err("SV1*HC:99213*100.00*UN*ONE") {
        EdiError::InvalidNumber {
            segment,
            index: 4,
            value,
        } => {
            assert_eq!((segment.as_str(), value.as_str()), ("SV1", "ONE"));
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn clp() {
    assert_eq!(
        parse("CLP*CLAIM001*1*150.00*120.00*30.00*12*PAYERREF001*11*1"),
        Segment::Clp(Clp {
            claim_id: "CLAIM001",
            status_code: "1",
            total_charge: 150.0,
            paid: 120.0,
            patient_responsibility: Some(30.0),
        })
    );
}

#[test]
fn clp_zero_paid_and_missing_patient_responsibility() {
    let Segment::Clp(clp) = parse("CLP*CLAIM201*4*95.00*0") else {
        panic!("not CLP")
    };
    assert_eq!(clp.paid, 0.0);
    assert_eq!(clp.patient_responsibility, None);
}

#[test]
fn svc() {
    assert_eq!(
        parse("SVC*HC:99213*100.00*80.00**1"),
        Segment::Svc(Svc {
            qualifier: Some("HC"),
            procedure_code: "99213",
            charge: 100.0,
            paid: 80.0
        })
    );
}

#[test]
fn trailers() {
    assert_eq!(
        parse("SE*29*0001"),
        Segment::Se(Se {
            segment_count: 29,
            control_number: "0001"
        })
    );
    assert_eq!(
        parse("GE*1*1"),
        Segment::Ge(Ge {
            transaction_count: 1,
            control_number: "1"
        })
    );
    assert_eq!(
        parse("IEA*1*000000001"),
        Segment::Iea(Iea {
            group_count: 1,
            control_number: "000000001"
        })
    );
}

#[test]
fn unknown_segments_keep_their_parts() {
    assert_eq!(
        parse("DTM*405*20261017"),
        Segment::Unknown {
            id: "DTM",
            elements: vec!["405", "20261017"]
        }
    );
    assert_eq!(
        parse("LX*1"),
        Segment::Unknown {
            id: "LX",
            elements: vec!["1"]
        }
    );
    assert_eq!(
        parse("HL*1**20*1"),
        Segment::Unknown {
            id: "HL",
            elements: vec!["1", "", "20", "1"]
        }
    );
}

#[test]
fn unknown_elements_are_moved_not_copied() {
    let input = String::from("CAS*PR*2*20.00");
    let range = input.as_bytes().as_ptr_range();
    let Segment::Unknown { id, elements } = parse(&input) else {
        panic!("not Unknown")
    };
    assert!(range.contains(&id.as_ptr()));
    assert!(elements.iter().all(|e| range.contains(&e.as_ptr())));
}

#[test]
fn id_matches_raw_id_for_every_variant() {
    let isa = isa('*', '^', ':', '~');
    for s in [
        isa.trim_end_matches('~'),
        "GS*HC*S*R*20261017*1200*1*X*005010",
        "ST*835*0001",
        "BPR*I*1.00",
        "NM1*QC*1*DOE",
        "CLM*C1*1.00",
        "SV1*HC:1*1.00",
        "CLP*C1*1*1.00*1.00",
        "SVC*HC:1*1.00*1.00",
        "SE*2*0001",
        "GE*1*1",
        "IEA*1*000000001",
        "REF*EA*X",
    ] {
        assert_eq!(parse(s).id(), raw(s).id, "{s:?}");
    }
}

#[test]
fn non_numeric_clp_amount() {
    match parse_err("CLP*CLAIM900*1*12A.00*12.00*0") {
        EdiError::InvalidNumber {
            segment,
            index,
            value,
        } => {
            assert_eq!(
                (segment.as_str(), index, value.as_str()),
                ("CLP", 3, "12A.00")
            );
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn non_numeric_count() {
    assert!(matches!(
        parse_err("SE*ten*0001"),
        EdiError::InvalidNumber { ref segment, index: 1, .. } if segment == "SE"
    ));
}

#[test]
fn missing_required_element() {
    assert!(matches!(
        parse_err("CLM*CLAIM001"),
        EdiError::MissingElement { ref segment, index: 2 } if segment == "CLM"
    ));
}

#[test]
fn empty_required_element() {
    assert!(matches!(
        parse_err("CLM**150.00"),
        EdiError::MissingElement { ref segment, index: 1 } if segment == "CLM"
    ));
    assert!(matches!(
        parse_err("SVC*HC:99213**80.00"),
        EdiError::MissingElement { ref segment, index: 2 } if segment == "SVC"
    ));
}

#[test]
fn required_helper() {
    let r = raw("REF*EA**X");
    assert_eq!(required(&r, 1).unwrap(), "EA");
    assert!(matches!(
        required(&r, 2),
        Err(EdiError::MissingElement { index: 2, .. })
    ));
    assert!(matches!(
        required(&r, 9),
        Err(EdiError::MissingElement { index: 9, .. })
    ));
}

#[test]
fn parse_amount_accepts_x12_decimals() {
    for (s, v) in [
        ("150.00", 150.0),
        ("0", 0.0),
        ("-5", -5.0),
        ("225.5", 225.5),
        (".5", 0.5),
        ("7.", 7.0),
    ] {
        assert_approx(parse_amount("CLM", 2, s).unwrap(), v);
    }
}

#[test]
fn parse_amount_rejects_everything_else() {
    for s in [
        "", "-", ".", "12A.00", "1e5", "NaN", "inf", "-inf", "+5", "1.2.3", " 1", "1,000",
    ] {
        match parse_amount("CLM", 2, s) {
            Err(EdiError::InvalidNumber {
                segment,
                index: 2,
                value,
            }) => {
                assert_eq!((segment.as_str(), value.as_str()), ("CLM", s));
            }
            other => panic!("{s:?}: expected InvalidNumber, got {other:?}"),
        }
    }
}

#[test]
fn parse_count_digits_only() {
    assert_eq!(parse_count("SE", 1, "29").unwrap(), 29);
    assert_eq!(parse_count("SE", 1, "0").unwrap(), 0);
    for s in ["", "-1", "1.0", "x", "+3"] {
        assert!(
            matches!(parse_count("SE", 1, s), Err(EdiError::InvalidNumber { .. })),
            "{s:?}"
        );
    }
}
