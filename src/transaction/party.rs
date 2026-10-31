//! A person or organization from an `NM1` segment.

use serde::Serialize;

/// A person or organization from an `NM1` segment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Party<'a> {
    /// NM103 (can be blank).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<&'a str>,
    /// NM104 (absent for organizations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<&'a str>,
    /// NM109 (NPI, member ID, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<&'a str>,
}
