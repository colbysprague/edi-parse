//! A person or organization from an `NM1` segment.

use serde::Serialize;

use crate::error::EdiError;
use crate::segment::Nm1;

/// A person or organization from an `NM1` segment. `NM102` decides which name
/// fields are filled in.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Party<'a> {
    /// NM103 when the party is a person (can be blank).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<&'a str>,
    /// NM104 when the party is a person.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<&'a str>,
    /// NM103 when the party is an organization (can be blank).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_name: Option<&'a str>,
    /// NM109 (NPI, member ID, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<&'a str>,
}

impl<'a> Party<'a> {
    /// Build a party from `nm1`, using `NM102` to tell a person (`1`) from an
    /// organization (`2`).
    ///
    /// Errors with [`EdiError::InvalidEntityType`] for any other `NM102`.
    pub fn from_nm1(nm1: &Nm1<'a>) -> Result<Self, EdiError> {
        let (last_name, first_name, organization_name) = match nm1.entity_type {
            "1" => (nm1.last_name, nm1.first_name, None),
            "2" => (None, None, nm1.last_name),
            other => {
                return Err(EdiError::InvalidEntityType {
                    value: other.to_string(),
                })
            }
        };

        Ok(Party {
            last_name,
            first_name,
            organization_name,
            id: nm1.id,
        })
    }
}
