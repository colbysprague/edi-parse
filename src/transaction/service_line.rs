//! A single service line, shared by 837 claims and 835 remits.

use crate::segment::Procedure;
use serde::Serialize;

/// One service line: an `SV1`, `SV2` or `SV3` (837) or an `SVC` (835).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ServiceLine<'a> {
    /// `SV2` only: the revenue code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revenue_code: Option<&'a str>,
    /// The code alone, without its qualifier, modifiers or description. `None`
    /// for an `SV2` that has only a revenue code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procedure_code: Option<&'a str>,
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
    pub fn new(procedure: Option<Procedure<'a>>, charge: f64, paid: Option<f64>) -> Self {
        let (procedure_code, modifiers, description) =
            procedure.map_or((None, Vec::new(), None), |procedure| {
                (
                    Some(procedure.code),
                    procedure.modifiers,
                    procedure.description,
                )
            });

        ServiceLine {
            revenue_code: None,
            procedure_code,
            modifiers,
            description,
            charge,
            paid,
        }
    }
}
