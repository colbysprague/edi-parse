//! Tasks 1 and 2: split raw text into segments and elements without copying.
//!
//! X12 uses four delimiters, all declared by the fixed-width ISA header:
//!
//! ```text
//! ISA*00*          *00*          *ZZ*SENDERID       *ZZ*RECEIVERID     *261017*1200*^*00501*000000001*0*T*:~
//! ```
//!
//! Every ISA element is fixed width, so the delimiters sit at fixed byte offsets:
//!
//! | offset | meaning                         | above |
//! |--------|---------------------------------|-------|
//! | 3      | element separator               | `*`   |
//! | 82     | repetition separator (ISA11)    | `^`   |
//! | 104    | component separator (ISA16)     | `:`   |
//! | 105    | segment terminator              | `~`   |

use crate::error::EdiError;
use std::collections::{HashMap, HashSet};

/// Length of a complete ISA segment, including its segment terminator.
pub const ISA_LEN: usize = 106;
/// Byte offset of the element separator in the ISA header.
pub const ELEMENT_SEP_POS: usize = 3;
/// Byte offset of ISA11, the repetition separator.
pub const REPETITION_SEP_POS: usize = 82;
/// Byte offset of ISA16, the component (sub-element) separator.
pub const COMPONENT_SEP_POS: usize = 104;
/// Byte offset of the segment terminator.
pub const SEGMENT_TERM_POS: usize = 105;

/// The four delimiters of an interchange. `Copy` because it's four `char`s:
/// pass it around by value or by `&`, never clone it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delimiters {
    /// Separates elements within a segment (usually `*`).
    pub element: char,
    /// Separates repeated values within one element (ISA11, usually `^`).
    pub repetition: char,
    /// Separates components of a composite element, e.g. `HC:99213` (ISA16, usually `:`).
    pub component: char,
    /// Ends a segment (usually `~`).
    pub segment: char,
}

impl Delimiters {
    /// The delimiters most files use: `*`, `^`, `:`, `~`.
    pub const STANDARD: Delimiters = Delimiters {
        element: '*',
        repetition: '^',
        component: ':',
        segment: '~',
    };

    /// Read the delimiters out of the ISA header at the start of `input`.
    ///
    /// Rules:
    /// - Leading whitespace (blank lines, `\r\n`) before `ISA` is ignored.
    /// - If the text doesn't start with `ISA` → [`EdiError::MissingIsa`].
    /// - If it starts with `ISA` but has fewer than [`ISA_LEN`] bytes →
    ///   [`EdiError::TruncatedIsa`] (`found` = bytes available after trimming).
    /// - The four delimiters must be distinct, and the element, component, and
    ///   segment delimiters must not be ASCII alphanumeric (older files put `U`
    ///   in ISA11) → otherwise [`EdiError::InvalidDelimiters`].
    pub fn detect(input: &str) -> Result<Delimiters, EdiError> {
        println!("\x1b[33mInput is: {input}\x1b[0m");

        let trimmed_header: &str = input.trim_start_matches(['\n', '\r', ' ']);

        if let Some(value) = validate_header(trimmed_header) {
            println!("\x1b[33mFOUND INVALID HEADER\x1b[0m");
            return value;
        }

        let delim_pos = [
            ELEMENT_SEP_POS,
            REPETITION_SEP_POS,
            COMPONENT_SEP_POS,
            SEGMENT_TERM_POS,
        ];

        let delims @ [element, repetition, component, segment] =
            delim_pos.map(|pos| trimmed_header.as_bytes()[pos] as char);

        let unique = HashSet::from(delims);
        if unique.len() != delim_pos.len() {
            return Err(EdiError::InvalidDelimiters(format!(
                "delimeters must be 4 distinct values, but found {:?}",
                delims
            )));
        }

        if [element, component, segment]
            .iter()
            .any(|c| c.is_ascii_alphanumeric())
        {
            return Err(EdiError::InvalidDelimiters(format!(
                "element, component, segment must be non-ascii-alphanumeric but found {:?}",
                delims
            )));
        }

        println!(
            "\x1b[33melement: {element} repetition: {repetition} component: {component} segment: {segment}\x1b[0m"
        );

        return Ok(Delimiters {
            element,
            repetition,
            component,
            segment,
        });
    }
}

fn validate_header(trimmed_header: &str) -> Option<Result<Delimiters, EdiError>> {
    if !trimmed_header.starts_with("ISA") {
        return Some(Err(EdiError::MissingIsa));
    }

    // Validate ISA header is long enough to parse delims
    let isa_len = trimmed_header.len();
    if isa_len < ISA_LEN {
        return Some(Err(EdiError::TruncatedIsa {
            expected: (ISA_LEN),
            found: (isa_len),
        }));
    }

    None
}

/// One segment, split into its ID and elements. Every `&'a str` points into
/// the original input; nothing is copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSegment<'a> {
    /// Segment ID, e.g. `"CLM"`.
    pub id: &'a str,
    /// Elements after the ID. `elements[0]` is X12 element 01 (e.g. `CLM01`).
    /// Empty elements (`**`) are kept as `""` so positions stay correct.
    pub elements: Vec<&'a str>,
}

impl<'a> RawSegment<'a> {
    /// Element by its X12 number (1-based: `element(1)` is `XX01`).
    ///
    /// Returns `None` when `n` is 0, out of range, or the element is empty.
    ///
    /// Note the return type: `&'a str`, not `&str` tied to `&self`. The slice
    /// borrows the *input*, so it stays valid after this `RawSegment` is dropped.
    pub fn element(&self, n: usize) -> Option<&'a str> {
        // if segment is 0
        if n == 0 {
            return None;
        }

        // if segment is out of range
        if n > self.elements.len() {
            return None;
        }

        // if segment is blank
        let out = self.elements[n - 1];
        if out.eq("") {
            return None;
        }

        return Some(out);
    }
}

/// Split `input` into segments using known delimiters.
///
/// - Splits on `delims.segment`, then each segment on `delims.element`.
/// - Strips `\r` and `\n` from the start and end of each segment (multi-line
///   files put a newline after each terminator).
/// - Skips segments that are empty after stripping (e.g. after the final `~`).
/// - A missing terminator on the last segment is fine.
/// - Each segment ID must be 2–3 uppercase ASCII letters/digits, otherwise
///   [`EdiError::InvalidSegmentId`] with the 0-based `position` among the
///   non-blank segments.
pub fn tokenize_with<'a>(
    input: &'a str,
    delims: &Delimiters,
) -> Result<Vec<RawSegment<'a>>, EdiError> {
    let segments: Vec<&str> = input
        .split(delims.segment)
        .map(|p| p.trim_matches(['\r', '\n']))
        .filter(|p| !p.is_empty())
        .collect();

    let mut out = Vec::new();

    for (idx, segment) in segments.iter().enumerate() {
        let elements: Vec<&str> = segment.split(delims.element).collect();
        let segment_id = get_valid_segment_id(elements.get(0).unwrap(), idx)?;

        out.push(RawSegment {
            id: segment_id,
            elements: elements[1..].to_vec(),
        })
    }

    return Ok(out);
}

fn get_valid_segment_id(segment_id: &str, position: usize) -> Result<&str, EdiError> {
    if !is_valid_segment_id(segment_id) {
        return Err(EdiError::InvalidSegmentId {
            id: segment_id.to_string(),
            position: position,
        });
    }

    return Ok(segment_id);
}

/// Validates that a given segment id
/// - is only uppercase ascii
/// - and is 2-3 chars long
fn is_valid_segment_id(id: &str) -> bool {
    (2..=3).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
}

/// Detect delimiters from the ISA header, then tokenize. Leading whitespace
/// before `ISA` is skipped, as in [`Delimiters::detect`].
pub fn tokenize<'a>(input: &'a str) -> Result<Vec<RawSegment<'a>>, EdiError> {
    let delims = &Delimiters::detect(input)?;
    return tokenize_with(input, delims);
}
