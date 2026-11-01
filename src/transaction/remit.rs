//! 835 types: [`Remit`] and the transaction set that holds them.

use super::{Adjustment, Party, Remark, ServiceLine};
use serde::Serialize;

/// An 835 claim payment: a `CLP` plus its `SVC` lines.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Remit<'a> {
    /// CLP01.
    pub claim_id: &'a str,
    /// CLP02.
    pub status_code: &'a str,
    /// CLP03.
    pub total_charge: f64,
    /// CLP04.
    pub paid: f64,
    /// CLP05.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_responsibility: Option<f64>,
    /// The `NM1*QC` that follows this `CLP`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient: Option<Party<'a>>,
    /// Claim-level `CAS` segments: those after the `CLP`, before any `SVC`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<Adjustment<'a>>,
    /// Claim-level `LQ` segments: those after the `CLP`, before any `SVC`.
    /// Version 008060X322 of the 835 puts them here.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remarks: Vec<Remark<'a>>,
    pub service_lines: Vec<ServiceLine<'a>>,
}

/// The contents of an 835 transaction set.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Remittance<'a> {
    /// ST02.
    pub control_number: &'a str,
    /// BPR02.
    pub total_payment: f64,
    pub remits: Vec<Remit<'a>>,
}
