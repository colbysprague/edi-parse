//! A single service line, shared by 837 claims and 835 remits.

use crate::segment::Procedure;
use serde::Serialize;

/// One service line: an `SV1` (837P) or an `SVC` (835).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ServiceLine<'a> {
    /// The code alone, without its qualifier, modifiers or description.
    pub procedure_code: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    pub charge: f64,
    /// `None` for 837 lines (nothing has been paid yet), `Some` for 835 lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid: Option<f64>,
}

impl<'a> ServiceLine<'a> {
    pub fn new(procedure: Procedure<'a>, charge: f64, paid: Option<f64>) -> Self {
        ServiceLine {
            procedure_code: procedure.code,
            modifiers: procedure.modifiers,
            description: procedure.description,
            charge,
            paid,
        }
    }
}
