//! 835 adjustments and remark codes, attached to remits and service lines.

use crate::segment::{Cas, Lq};
use serde::Serialize;

/// One adjustment: a single triplet of a `CAS`, plus that segment's group code.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Adjustment<'a> {
    /// CAS01.
    pub group_code: &'a str,
    pub reason_code: &'a str,
    pub amount: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<f64>,
}

impl<'a> Adjustment<'a> {
    /// One [`Adjustment`] per triplet of `cas`, in segment order.
    pub fn from_cas(cas: Cas<'a>) -> impl Iterator<Item = Self> {
        let group_code = cas.group_code;
        cas.triplets.into_iter().map(move |triplet| Adjustment {
            group_code,
            reason_code: triplet.reason_code,
            amount: triplet.amount,
            quantity: triplet.quantity,
        })
    }
}

/// A remark code from an `LQ` segment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Remark<'a> {
    /// LQ01: `HE` (RARC) or `RX` (NCPDP reject code).
    pub qualifier: &'a str,
    /// LQ02.
    pub code: &'a str,
}

impl<'a> From<Lq<'a>> for Remark<'a> {
    fn from(lq: Lq<'a>) -> Self {
        Remark {
            qualifier: lq.qualifier,
            code: lq.code,
        }
    }
}
