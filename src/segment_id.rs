//! X12 segment IDs, as they appear in the first element of each segment.
//!
//! Usable as `match` patterns: `match raw.id { segment_id::CLM => ..., _ => ... }`.
//! Always refer to them by path (`segment_id::CLM`), never via `use segment_id::*`;
//! see the note on bare identifiers in patterns in `Segment::from_raw`.

pub const ISA: &str = "ISA";
pub const GS: &str = "GS";
pub const ST: &str = "ST";
pub const BPR: &str = "BPR";
pub const NM1: &str = "NM1";
pub const CLM: &str = "CLM";
pub const SV1: &str = "SV1";
pub const SV2: &str = "SV2";
pub const SV3: &str = "SV3";
pub const CLP: &str = "CLP";
pub const SVC: &str = "SVC";
pub const SE: &str = "SE";
pub const GE: &str = "GE";
pub const IEA: &str = "IEA";
