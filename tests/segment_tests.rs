//! `Segment::from_raw`, `Segment::id`, and the parsing helpers.
//!
//! Raw segments are built by the test helper `raw` below (not the tokenizer),
//! so these tests only depend on `RawSegment::element`.

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
            procedure: Procedure {
                qualifier: Some("HC"),
                code: "99213",
                modifiers: Vec::new(),
                description: None,
            },
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
    assert_eq!(sv1.procedure.qualifier, Some("HC"));
    assert_eq!(sv1.procedure.code, "36415");
    assert_eq!(sv1.units, Some(2.0));
}

#[test]
fn sv1_without_component_separator_or_units() {
    let Segment::Sv1(sv1) = parse("SV1*99213*100.00") else {
        panic!("not SV1")
    };
    assert_eq!(sv1.procedure.qualifier, None);
    assert_eq!(sv1.procedure.code, "99213");
    assert_eq!(sv1.units, None);
}

#[test]
fn procedure_with_modifiers() {
    assert_eq!(
        Procedure::parse("HC:K0001:RR:KH:BR", ':'),
        Procedure {
            qualifier: Some("HC"),
            code: "K0001",
            modifiers: vec!["RR", "KH", "BR"],
            description: None,
        }
    );
}

#[test]
fn procedure_with_description_after_empty_modifiers() {
    assert_eq!(
        Procedure::parse("HC:J3490:::::Hep Lock see NDC#", ':'),
        Procedure {
            qualifier: Some("HC"),
            code: "J3490",
            modifiers: Vec::new(),
            description: Some("Hep Lock see NDC#"),
        }
    );
}

#[test]
fn procedure_with_four_modifiers_and_description() {
    let procedure = Procedure::parse("HC:99213:A:B:C:D:TEXT", ':');
    assert_eq!(procedure.modifiers, ["A", "B", "C", "D"]);
    assert_eq!(procedure.description, Some("TEXT"));
}

#[test]
fn procedure_ignores_components_after_the_description() {
    let procedure = Procedure::parse("HC:99213:::::TEXT:EXTRA", ':');
    assert_eq!(procedure.description, Some("TEXT"));
}

#[test]
fn procedure_skips_empty_modifier_positions() {
    let procedure = Procedure::parse("HC:99213::25", ':');
    assert_eq!(procedure.modifiers, ["25"]);
}

#[test]
fn sv2_reads_the_revenue_code_procedure_and_charge() {
    let Segment::Sv2(sv2) = parse("SV2*0305*HC:85025*13.39*UN*1") else {
        panic!("not SV2")
    };
    assert_eq!(sv2.revenue_code, Some("0305"));
    assert_eq!(sv2.procedure.unwrap().code, "85025");
    assert_eq!(sv2.charge, 13.39);
}

#[test]
fn sv2_without_a_procedure() {
    let Segment::Sv2(sv2) = parse("SV2*0120**20.00*DA*1") else {
        panic!("not SV2")
    };
    assert_eq!(sv2.procedure, None);
}

#[test]
fn sv2_requires_a_charge() {
    assert!(matches!(
        parse_err("SV2*0305*HC:85025"),
        EdiError::MissingElement { index: 3, .. }
    ));
}

#[test]
fn sv3_reads_the_procedure_and_charge() {
    let Segment::Sv3(sv3) = parse("SV3*AD:D2150*100****1") else {
        panic!("not SV3")
    };
    assert_eq!(sv3.procedure.qualifier, Some("AD"));
    assert_eq!(sv3.procedure.code, "D2150");
    assert_eq!(sv3.charge, 100.0);
}

#[test]
fn sv3_requires_a_procedure() {
    assert!(matches!(
        parse_err("SV3**100"),
        EdiError::MissingElement { index: 1, .. }
    ));
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
            procedure: Procedure {
                qualifier: Some("HC"),
                code: "99213",
                modifiers: Vec::new(),
                description: None,
            },
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
    let input = String::from("DTM*405*20261017");
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
        "CAS*CO*45*1.00",
        "LQ*HE*N130",
        "SE*2*0001",
        "GE*1*1",
        "IEA*1*000000001",
        "REF*EA*X",
    ] {
        assert_eq!(parse(s).id(), raw(s).id, "{s:?}");
    }
}

fn triplet(reason_code: &str, amount: f64, quantity: Option<f64>) -> CasTriplet<'_> {
    CasTriplet {
        reason_code,
        amount,
        quantity,
    }
}

#[test]
fn cas_single_triplet() {
    assert_eq!(
        parse("CAS*PR*2*20.00"),
        Segment::Cas(Cas {
            group_code: "PR",
            triplets: vec![triplet("2", 20.0, None)],
        })
    );
}

#[test]
fn cas_full_six_triplets() {
    assert_eq!(
        parse("CAS*CO*1*1.00*1*2*2.00*2*3*3.00*3*4*4.00*4*5*5.00*5*6*6.00*6"),
        Segment::Cas(Cas {
            group_code: "CO",
            triplets: vec![
                triplet("1", 1.0, Some(1.0)),
                triplet("2", 2.0, Some(2.0)),
                triplet("3", 3.0, Some(3.0)),
                triplet("4", 4.0, Some(4.0)),
                triplet("5", 5.0, Some(5.0)),
                triplet("6", 6.0, Some(6.0)),
            ],
        })
    );
}

#[test]
fn cas_omitted_trailing_triplets() {
    let Segment::Cas(cas) = parse("CAS*OA*23*5.00*1*45*2.50") else {
        panic!("not Cas")
    };
    assert_eq!(
        cas.triplets,
        [triplet("23", 5.0, Some(1.0)), triplet("45", 2.5, None)]
    );
}

#[test]
fn cas_empty_quantity() {
    let Segment::Cas(cas) = parse("CAS*PI*96*7.00**97*3.00") else {
        panic!("not Cas")
    };
    assert_eq!(
        cas.triplets,
        [triplet("96", 7.0, None), triplet("97", 3.0, None)]
    );
}

#[test]
fn cas_blank_triplet_between_others_is_skipped() {
    let Segment::Cas(cas) = parse("CAS*CR*1*1.00*****3*3.00") else {
        panic!("not Cas")
    };
    assert_eq!(
        cas.triplets,
        [triplet("1", 1.0, None), triplet("3", 3.0, None)]
    );
}

#[test]
fn cas_accepts_every_group_code_and_negative_amounts() {
    for code in ["CO", "PR", "OA", "PI", "CR"] {
        let s = format!("CAS*{code}*45*-5.00");
        let Segment::Cas(cas) = parse(&s) else {
            panic!("not Cas")
        };
        assert_eq!(cas.group_code, code);
        assert_eq!(cas.triplets, [triplet("45", -5.0, None)]);
    }
}

#[test]
fn cas_bad_group_code() {
    match parse_err("CAS*XX*45*5.00") {
        EdiError::InvalidGroupCode { value } => assert_eq!(value, "XX"),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn cas_missing_required_elements() {
    for (s, expected) in [
        ("CAS", 1),
        ("CAS**45*5.00", 1),
        ("CAS*CO", 2),
        ("CAS*CO**5.00", 2),
        ("CAS*CO*45", 3),
        ("CAS*CO*45*", 3),
        // A later triplet is optional, but can't be partly filled in.
        ("CAS*CO*45*5.00**94", 6),
        ("CAS*CO*45*5.00***5.00", 5),
        ("CAS*CO*45*5.00****1", 5),
    ] {
        match parse_err(s) {
            EdiError::MissingElement { segment, index } => {
                assert_eq!((segment.as_str(), index), ("CAS", expected), "{s:?}");
            }
            other => panic!("{s:?}: got {other:?}"),
        }
    }
}

#[test]
fn cas_non_numeric_amount_and_quantity() {
    for (s, expected_index, expected_value) in [
        ("CAS*CO*45*12A.00", 3, "12A.00"),
        ("CAS*CO*45*5.00*x", 4, "x"),
        ("CAS*CO*45*5.00**94*1e5", 6, "1e5"),
    ] {
        match parse_err(s) {
            EdiError::InvalidNumber {
                segment,
                index,
                value,
            } => assert_eq!(
                (segment.as_str(), index, value.as_str()),
                ("CAS", expected_index, expected_value),
                "{s:?}"
            ),
            other => panic!("{s:?}: got {other:?}"),
        }
    }
}

#[test]
fn lq() {
    assert_eq!(
        parse("LQ*HE*N130"),
        Segment::Lq(Lq {
            qualifier: "HE",
            code: "N130",
        })
    );
}

#[test]
fn lq_missing_code() {
    match parse_err("LQ*HE") {
        EdiError::MissingElement { segment, index } => {
            assert_eq!((segment.as_str(), index), ("LQ", 2));
        }
        other => panic!("got {other:?}"),
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
