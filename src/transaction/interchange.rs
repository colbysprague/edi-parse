//! The top-level parse result: an [`Interchange`] of [`Transaction`]s.

use super::{ClaimSet, Remittance};
use serde::Serialize;

/// One `ST`..`SE` transaction set.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Transaction<'a> {
    Claims(ClaimSet<'a>),
    Remittance(Remittance<'a>),
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
