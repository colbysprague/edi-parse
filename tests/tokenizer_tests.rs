//! `tokenize_with` and `RawSegment::element`.
//! Delimiters are given explicitly here; `delimiter_tests` covers detecting them.

use edi_parse::{tokenize_with, Delimiters, EdiError, RawSegment};

const STD: Delimiters = Delimiters::STANDARD;

#[test]
fn splits_segments_and_elements() {
    let segs = tokenize_with("ST*837*0001~SE*2*0001~", &STD).unwrap();
    assert_eq!(
        segs,
        vec![
            RawSegment {
                id: "ST",
                elements: vec!["837", "0001"]
            },
            RawSegment {
                id: "SE",
                elements: vec!["2", "0001"]
            },
        ]
    );
}

#[test]
fn keeps_empty_elements_so_positions_stay_correct() {
    let segs = tokenize_with("NM1*IL*1*DOE*JANE****MI*MEMBER001~", &STD).unwrap();
    assert_eq!(segs.len(), 1);
    assert_eq!(
        segs[0].elements,
        vec!["IL", "1", "DOE", "JANE", "", "", "", "MI", "MEMBER001"]
    );
}

#[test]
fn keeps_trailing_empty_element() {
    let segs = tokenize_with("CLM*A*~", &STD).unwrap();
    assert_eq!(segs[0].elements, vec!["A", ""]);
}

#[test]
fn segment_with_no_elements() {
    let segs = tokenize_with("LX~", &STD).unwrap();
    assert_eq!(
        segs,
        vec![RawSegment {
            id: "LX",
            elements: vec![]
        }]
    );
}

#[test]
fn strips_line_breaks_between_segments() {
    let segs = tokenize_with("ST*837*0001~\nBHT*0019~\r\nSE*3*0001~\r\n", &STD).unwrap();
    let ids: Vec<&str> = segs.iter().map(|s| s.id).collect();
    assert_eq!(ids, ["ST", "BHT", "SE"]);
    // No stray '\r' left on the last element of any segment.
    assert_eq!(segs[2].elements, vec!["3", "0001"]);
}

#[test]
fn final_terminator_is_optional() {
    let segs = tokenize_with("ST*837*0001~SE*2*0001", &STD).unwrap();
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[1].elements, vec!["2", "0001"]);
}

#[test]
fn skips_blank_segments() {
    let segs = tokenize_with("ST*837*0001~~\n~SE*2*0001~\n\n", &STD).unwrap();
    assert_eq!(segs.len(), 2);
}

#[test]
fn empty_input_is_no_segments() {
    assert_eq!(tokenize_with("", &STD).unwrap(), vec![]);
    assert_eq!(tokenize_with("\r\n", &STD).unwrap(), vec![]);
}

#[test]
fn uses_the_given_delimiters_not_star_and_tilde() {
    let pipes = Delimiters {
        element: '|',
        repetition: '{',
        component: '>',
        segment: '^',
    };
    let segs = tokenize_with("ST|837|0001^SV1|HC>99213|1.00^", &pipes).unwrap();
    assert_eq!(
        segs[0],
        RawSegment {
            id: "ST",
            elements: vec!["837", "0001"]
        }
    );
    // The component separator is not split here; that's the segment parser's job.
    assert_eq!(segs[1].elements, vec!["HC>99213", "1.00"]);

    // '*' and '~' are ordinary data under these delimiters.
    let segs = tokenize_with("NM1|85|2|A*B~C^", &pipes).unwrap();
    assert_eq!(segs[0].elements, vec!["85", "2", "A*B~C"]);
}

#[test]
fn newline_as_segment_terminator() {
    let nl = Delimiters {
        segment: '\n',
        ..STD
    };
    let segs = tokenize_with("ST*837*0001\nSE*2*0001\n", &nl).unwrap();
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[1].elements, vec!["2", "0001"]);
}

#[test]
fn slices_point_into_the_input_zero_copy() {
    let input = String::from("ST*837*0001~NM1*IL*1*DOE*JANE~SE*3*0001~");
    let range = input.as_bytes().as_ptr_range();
    for seg in tokenize_with(&input, &STD).unwrap() {
        assert!(
            range.contains(&seg.id.as_ptr()),
            "id {:?} was copied",
            seg.id
        );
        for el in seg.elements.iter().filter(|e| !e.is_empty()) {
            assert!(range.contains(&el.as_ptr()), "element {el:?} was copied");
        }
    }
}

#[test]
fn slices_outlive_the_segment_vec() {
    let input = "ST*837*0001~";
    let id;
    let control;
    {
        let segs = tokenize_with(input, &STD).unwrap();
        id = segs[0].id;
        control = segs[0].element(2).unwrap();
    } // `segs` is dropped here; the slices borrow `input`, not `segs`.
    assert_eq!((id, control), ("ST", "0001"));
}

#[test]
fn element_is_one_based() {
    let seg = RawSegment {
        id: "NM1",
        elements: vec!["IL", "1", "DOE", "", "MI"],
    };
    assert_eq!(seg.element(1), Some("IL"));
    assert_eq!(seg.element(3), Some("DOE"));
    assert_eq!(seg.element(5), Some("MI"));
}

#[test]
fn element_none_for_zero_out_of_range_or_empty() {
    let seg = RawSegment {
        id: "NM1",
        elements: vec!["IL", "1", "DOE", "", "MI"],
    };
    assert_eq!(seg.element(0), None);
    assert_eq!(seg.element(4), None, "empty element should be None");
    assert_eq!(seg.element(6), None);
    assert_eq!(seg.element(usize::MAX), None);
}

#[test]
fn rejects_empty_segment_id() {
    let err = tokenize_with("ST*837*0001~*BAD~", &STD).unwrap_err();
    assert!(
        matches!(err, EdiError::InvalidSegmentId { position: 1, ref id } if id.is_empty()),
        "got {err:?}"
    );
}

#[test]
fn rejects_bad_segment_ids() {
    for (input, bad) in [
        ("TOOLONG*1~", "TOOLONG"),
        ("S*1~", "S"),
        ("st*1~", "st"),
        ("S T*1~", "S T"),
    ] {
        let err = tokenize_with(input, &STD).unwrap_err();
        assert!(
            matches!(err, EdiError::InvalidSegmentId { position: 0, ref id } if id == bad),
            "{input:?}: got {err:?}"
        );
    }
}

#[test]
fn accepts_ids_with_digits() {
    let segs = tokenize_with("NM1*85~SV1*HC:1~N1*PR~", &STD).unwrap();
    let ids: Vec<&str> = segs.iter().map(|s| s.id).collect();
    assert_eq!(ids, ["NM1", "SV1", "N1"]);
}
