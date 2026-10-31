//! The top-level parse result: an [`Interchange`] of [`Transaction`]s.

use super::{ClaimSet, Remittance};
use crate::Claim;
use crate::Remit;
use serde::Serialize;

/// One `ST`..`SE` transaction set.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Transaction<'a> {
    Claims(ClaimSet<'a>),
    Remittance(Remittance<'a>),
}

impl<'a> Transaction<'a> {
    pub fn as_claim_set(&self) -> Option<&ClaimSet<'a>> {
        match self {
            Transaction::Claims(set) => Some(set),
            _ => None,
        }
    }

    pub fn claims(&self) -> &[Claim<'a>] {
        self.as_claim_set().map_or(&[], |s| &s.claims)
    }

    pub fn as_remittance(&self) -> Option<&Remittance<'a>> {
        match self {
            Transaction::Remittance(r) => Some(r),
            _ => None,
        }
    }

    pub fn remits(&self) -> &[Remit<'a>] {
        self.as_remittance().map_or(&[], |r| &r.remits)
    }
}

/// A whole parsed file: one `ISA`..`IEA` interchange.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Interchange<'a> {
    /// ISA06 (trimmed).
    pub sender_id: &'a str,
    /// ISA08 (trimmed).
    pub receiver_id: &'a str,
    /// ISA13.
    pub control_number: &'a str,
    /// Every transaction set from every functional group, in file order.
    pub transactions: Vec<Transaction<'a>>,
}
