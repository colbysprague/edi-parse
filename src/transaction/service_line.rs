//! A single service line, shared by 837 claims and 835 remits.

use serde::Serialize;

/// One service line: an `SV1` (837P) or an `SVC` (835).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ServiceLine<'a> {
    pub procedure_code: &'a str,
    pub charge: f64,
    /// `None` for 837 lines (nothing has been paid yet), `Some` for 835 lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid: Option<f64>,
}
