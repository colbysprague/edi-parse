//! Task 2: `Delimiters::detect` and `tokenize`.

mod common;

use common::*;
use edi_parse::{tokenize, Delimiters, EdiError};

#[test]
fn detects_standard_delimiters() {
    let input = isa('*', '^', ':', '~') + "GS*HC~";
    assert_eq!(Delimiters::detect(&input).unwrap(), Delimiters::STANDARD);
}

#[test]
fn detects_from_exactly_one_isa() {
    let input = isa('*', '^', ':', '~');
    assert_eq!(input.len(), 106);
    assert_eq!(Delimiters::detect(&input).unwrap(), Delimiters::STANDARD);
}

#[test]
fn detects_non_standard_delimiters() {
    let input = isa('|', '{', '>', '^');
    assert_eq!(
        Delimiters::detect(&input).unwrap(),
        Delimiters {
            element: '|',
            repetition: '{',
            component: '>',
            segment: '^'
        }
    );
}

#[test]
fn detects_pipe_sample_file() {
    assert_eq!(
        Delimiters::detect(&sample(S_837_PIPE)).unwrap(),
        Delimiters {
            element: '|',
            repetition: '{',
            component: '>',
            segment: '^'
        }
    );
}

#[test]
fn detects_every_standard_sample() {
    for name in [S_837_MULTI, S_835_MULTI, S_835_CRLF] {
        assert_eq!(
            Delimiters::detect(&sample(name)).unwrap(),
            Delimiters::STANDARD,
            "{name}"
        );
    }
}

#[test]
fn newline_segment_terminator() {
    let input = isa('*', '^', ':', '\n');
    assert_eq!(Delimiters::detect(&input).unwrap().segment, '\n');
}

#[test]
fn ignores_leading_whitespace() {
    let input = format!("\r\n\n  {}", isa('*', '^', ':', '~'));
    assert_eq!(Delimiters::detect(&input).unwrap(), Delimiters::STANDARD);
}

#[test]
fn empty_input_is_missing_isa() {
    assert!(matches!(Delimiters::detect(""), Err(EdiError::MissingIsa)));
    assert!(matches!(
        Delimiters::detect("\n\n"),
        Err(EdiError::MissingIsa)
    ));
}

#[test]
fn non_isa_start_is_missing_isa() {
    let input = isa('*', '^', ':', '~').replacen("ISA", "GSX", 1);
    assert!(matches!(
        Delimiters::detect(&input),
        Err(EdiError::MissingIsa)
    ));
    assert!(matches!(
        Delimiters::detect("GS*HC*X~"),
        Err(EdiError::MissingIsa)
    ));
}

#[test]
fn short_isa_is_truncated() {
    let full = isa('*', '^', ':', '~');
    let short = &full[..105]; // one byte short: no terminator
    match Delimiters::detect(short) {
        Err(EdiError::TruncatedIsa {
            expected: 106,
            found: 105,
        }) => {}
        other => panic!("expected TruncatedIsa {{ expected: 106, found: 105 }}, got {other:?}"),
    }
}

#[test]
fn truncated_isa_invalid_file() {
    let input = invalid("truncated_isa.edi");
    assert!(
        matches!(
            Delimiters::detect(&input),
            Err(EdiError::TruncatedIsa { expected: 106, .. })
        ),
        "got {:?}",
        Delimiters::detect(&input)
    );
}

#[test]
fn duplicate_delimiters_are_rejected() {
    for input in [
        isa('*', '^', ':', '*'), // element == segment
        isa('*', ':', ':', '~'), // repetition == component
        isa('*', '^', '~', '~'), // component == segment
    ] {
        assert!(
            matches!(
                Delimiters::detect(&input),
                Err(EdiError::InvalidDelimiters(_))
            ),
            "{input:?}"
        );
    }
}

#[test]
fn alphanumeric_delimiters_are_rejected() {
    let input = isa('*', '^', 'Z', '~');
    assert!(matches!(
        Delimiters::detect(&input),
        Err(EdiError::InvalidDelimiters(_))
    ));
}

#[test]
fn tokenize_detects_then_splits() {
    let input = isa('|', '{', '>', '^') + "GS|HC|A^ST|837|0001^";
    let segs = tokenize(&input).unwrap();
    let ids: Vec<&str> = segs.iter().map(|s| s.id).collect();
    assert_eq!(ids, ["ISA", "GS", "ST"]);
    assert_eq!(segs[1].elements, vec!["HC", "A"]);
}

#[test]
fn isa_has_sixteen_elements_and_isa16_is_the_component_separator() {
    let input = sample(S_837_MULTI);
    let segs = tokenize(&input).unwrap();
    assert_eq!(segs[0].id, "ISA");
    assert_eq!(segs[0].elements.len(), 16);
    assert_eq!(segs[0].element(16), Some(":"));
    assert_eq!(
        segs[0].element(6),
        Some("SENDERID       "),
        "ISA06 is fixed-width, padded"
    );
}

#[test]
fn tokenize_pipe_sample() {
    let input = sample(S_837_PIPE);
    let segs = tokenize(&input).unwrap();
    assert_eq!(segs.len(), 24);
    assert_eq!(segs[1].id, "GS");
    assert_eq!(segs[1].element(1), Some("HC"));
    assert!(segs
        .iter()
        .all(|s| !s.elements.iter().any(|e| e.contains('|'))));
}

#[test]
fn tokenize_multi_line_sample() {
    let input = sample(S_835_MULTI);
    let segs = tokenize(&input).unwrap();
    assert_eq!(segs.len(), 27);
    for seg in &segs {
        assert!(!seg.id.contains('\n'), "{:?}", seg.id);
        assert!(seg.elements.iter().all(|e| !e.contains('\n')), "{seg:?}");
    }
}

#[test]
fn tokenize_crlf_sample() {
    let input = sample(S_835_CRLF);
    let segs = tokenize(&input).unwrap();
    assert_eq!(segs.len(), 17);
    for seg in &segs {
        assert!(seg.elements.iter().all(|e| !e.contains('\r')), "{seg:?}");
    }
    assert_eq!(segs.last().unwrap().id, "IEA");
}

#[test]
fn tokenize_single_line_sample() {
    let input = sample(S_837_MULTI);
    let segs = tokenize(&input).unwrap();
    assert_eq!(segs.len(), 33);
    assert_eq!(segs.last().unwrap().id, "IEA");
}

#[test]
fn tokenize_propagates_detect_errors() {
    assert!(matches!(tokenize("GS*HC~"), Err(EdiError::MissingIsa)));
    assert!(matches!(
        tokenize(&invalid("truncated_isa.edi")),
        Err(EdiError::TruncatedIsa { .. })
    ));
}
