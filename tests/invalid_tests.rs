//! Task 4: error handling for malformed files: `parse_segments`,
//! `check_envelope`, and the `?`-driven `parse_str` pipeline.

mod common;

use common::*;
use edi_parse::{check_envelope, parse_segments, parse_str, EdiError, Segment};

fn segments_err(input: &str) -> EdiError {
    match parse_segments(input).and_then(|segs| check_envelope(&segs)) {
        Ok(()) => panic!("expected an error"),
        Err(e) => e,
    }
}

// ---- one test per file in data/invalid/ ----------------------------------

#[test]
fn truncated_isa() {
    let input = invalid("truncated_isa.edi");
    assert!(matches!(
        parse_segments(&input),
        Err(EdiError::TruncatedIsa { .. })
    ));
    assert!(matches!(
        parse_str(&input),
        Err(EdiError::TruncatedIsa { .. })
    ));
}

#[test]
fn se_count_mismatch() {
    let input = invalid("se_count_mismatch.edi");
    let segs = parse_segments(&input).expect("segments themselves are fine");
    match check_envelope(&segs) {
        Err(EdiError::SegmentCountMismatch {
            control_number,
            declared,
            actual,
        }) => {
            assert_eq!((control_number.as_str(), declared, actual), ("0901", 99, 9));
        }
        other => panic!("got {other:?}"),
    }
    assert!(matches!(
        parse_str(&input),
        Err(EdiError::SegmentCountMismatch { .. })
    ));
}

#[test]
fn clp_non_numeric_amount() {
    let input = invalid("clp_bad_amount.edi");
    match parse_str(&input) {
        Err(EdiError::InvalidNumber {
            segment,
            index,
            value,
        }) => {
            assert_eq!(
                (segment.as_str(), index, value.as_str()),
                ("CLP", 3, "12A.00")
            );
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn missing_iea() {
    let input = invalid("missing_iea.edi");
    let segs = parse_segments(&input).expect("segments themselves are fine");
    assert!(matches!(check_envelope(&segs), Err(EdiError::MissingIea)));
    assert!(matches!(parse_str(&input), Err(EdiError::MissingIea)));
}

#[test]
fn every_invalid_file_fails_to_parse() {
    let mut count = 0;
    for entry in std::fs::read_dir(invalid_dir()).unwrap() {
        let path = entry.unwrap().path();
        let input = std::fs::read_to_string(&path).unwrap();
        assert!(
            parse_str(&input).is_err(),
            "{} parsed but should not",
            path.display()
        );
        count += 1;
    }
    assert!(count >= 4, "expected at least 4 files in data/invalid");
}

// ---- valid input passes -----------------------------------------------------

#[test]
fn every_sample_passes_the_envelope_check() {
    for name in ALL_SAMPLES {
        let input = sample(name);
        let segs = parse_segments(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
        check_envelope(&segs).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn parse_segments_returns_typed_segments() {
    let input = sample(S_837_MULTI);
    let segs = parse_segments(&input).unwrap();
    assert_eq!(segs.len(), 33);
    assert!(matches!(segs[0], Segment::Isa(_)));
    assert!(matches!(segs[2], Segment::St(_)));
    assert!(matches!(segs[32], Segment::Iea(_)));
    assert_eq!(
        segs.iter().filter(|s| matches!(s, Segment::Clm(_))).count(),
        3
    );
}

#[test]
fn parse_segments_uses_detected_component_separator() {
    let input = sample(S_837_PIPE);
    let segs = parse_segments(&input).unwrap();
    let codes: Vec<&str> = segs
        .iter()
        .filter_map(|s| match s {
            Segment::Sv1(sv1) => Some(sv1.procedure_code),
            _ => None,
        })
        .collect();
    assert_eq!(codes, ["99213", "36415", "85025"]);
}

#[test]
fn minimal_envelope_is_ok() {
    let input = envelope("HC", &["ST*837*0001", "SE*2*0001"]);
    check_envelope(&parse_segments(&input).unwrap()).unwrap();
}

#[test]
fn multiple_transactions_each_checked() {
    let ok = envelope(
        "HC",
        &[
            "ST*837*0001",
            "BHT*0019",
            "SE*3*0001",
            "ST*837*0002",
            "SE*2*0002",
        ],
    );
    check_envelope(&parse_segments(&ok).unwrap()).unwrap();

    let bad = envelope(
        "HC",
        &[
            "ST*837*0001",
            "SE*2*0001",
            "ST*837*0002",
            "LX*1",
            "SE*2*0002",
        ],
    );
    match segments_err(&bad) {
        EdiError::SegmentCountMismatch {
            control_number,
            declared: 2,
            actual: 3,
        } => {
            assert_eq!(control_number, "0002");
        }
        other => panic!("got {other:?}"),
    }
}

// ---- other envelope errors ------------------------------------------------

#[test]
fn empty_segment_list_is_missing_isa() {
    assert!(matches!(check_envelope(&[]), Err(EdiError::MissingIsa)));
}

#[test]
fn first_segment_must_be_isa() {
    let segs = [
        Segment::Unknown {
            id: "GS",
            elements: vec!["HC"],
        },
        Segment::Unknown {
            id: "IEA",
            elements: vec![],
        },
    ];
    assert!(matches!(check_envelope(&segs), Err(EdiError::MissingIsa)));
}

#[test]
fn se_control_number_must_match_st() {
    let input = envelope("HC", &["ST*837*0001", "SE*2*0002"]);
    match segments_err(&input) {
        EdiError::ControlNumberMismatch {
            segment,
            expected,
            found,
        } => {
            assert_eq!(
                (segment.as_str(), expected.as_str(), found.as_str()),
                ("SE", "0001", "0002")
            );
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn st_without_se_is_unexpected_segment() {
    let input = envelope("HC", &["ST*837*0001", "BHT*0019"]);
    match segments_err(&input) {
        EdiError::UnexpectedSegment {
            found, expected, ..
        } => {
            assert_eq!(found, "GE");
            assert!(
                expected.contains("SE"),
                "expected mentions SE: {expected:?}"
            );
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn nested_st_is_unexpected_segment() {
    let input = envelope("HC", &["ST*837*0001", "ST*837*0002", "SE*2*0002"]);
    assert!(matches!(
        segments_err(&input),
        EdiError::UnexpectedSegment { ref found, position: 3, .. } if found == "ST"
    ));
}

#[test]
fn errors_have_readable_messages() {
    let input = invalid("se_count_mismatch.edi");
    let msg = parse_str(&input).unwrap_err().to_string();
    assert!(msg.contains("0901") && msg.contains("99"), "{msg}");
}
