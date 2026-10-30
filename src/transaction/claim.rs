//! 837P types: [`Claim`] and the transaction set that holds them.

use super::{Party, ServiceLine};
use serde::Serialize;

/// An 837P claim: a `CLM` plus its `SV1` lines.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Claim<'a> {
    /// CLM01.
    pub claim_id: &'a str,
    /// CLM02.
    pub total_charge: f64,
    /// The most recent `NM1*85` seen before this `CLM`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_provider: Option<Party<'a>>,
    /// The most recent `NM1*IL` seen before this `CLM`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscriber: Option<Party<'a>>,
    pub service_lines: Vec<ServiceLine<'a>>,
}

/// The contents of an 837 transaction set.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClaimSet<'a> {
    /// ST02.
    pub control_number: &'a str,
    pub claims: Vec<Claim<'a>>,
}
