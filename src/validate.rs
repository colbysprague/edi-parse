//! Business-rule validation through a trait.
//!
//! Validation never stops at the first problem: every rule is checked and all
//! failures are returned together.

use crate::error::ValidationError;
use crate::transaction::{Claim, Interchange, Remit, Remittance, Transaction};

/// Amounts within this distance are considered equal (floating point sums
/// like `0.1 + 0.2` are not exactly `0.3`).
pub const AMOUNT_TOLERANCE: f64 = 0.005;

/// Valid CLP02 claim status codes.
pub const CLAIM_STATUS_CODES: &[&str] = &["1", "2", "3", "4", "19", "20", "21", "22", "23", "25"];

/// Something that can check its own business rules.
pub trait Validate {
    /// `Ok(())` if every rule passes, otherwise every failure found.
    fn validate(&self) -> Result<(), Vec<ValidationError>>;

    /// Convenience wrapper. Provided by the trait: implementors get it free.
    fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }
}

/// Rules, in this order:
/// 1. `claim_id` non-empty → else `EmptyClaimId`.
/// 2. `total_charge` and every line charge ≥ 0 → else `NegativeAmount`
///    (`field`: `"total_charge"` or `"line_charge"`).
/// 3. At least one service line → else `NoServiceLines`.
/// 4. `total_charge` equals the sum of line charges (within
///    [`AMOUNT_TOLERANCE`]) → else `ChargeMismatch`. Skip if there are no lines.
impl Validate for Claim<'_> {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut validation_errors = Vec::new();
        let negative_amount_err = negative_amount_err();

        if self.claim_id.is_empty() {
            validation_errors.push(ValidationError::EmptyClaimId);
        }

        if is_neg_amount(self.total_charge) {
            validation_errors.push(negative_amount_err(
                self.claim_id.to_string(),
                "total_charge",
                self.total_charge,
            ))
        }

        if self.service_lines.is_empty() {
            validation_errors.push(ValidationError::NoServiceLines {
                claim_id: (self.claim_id.to_string()),
            })
        }

        for line in &self.service_lines {
            if is_neg_amount(line.charge) {
                validation_errors.push(negative_amount_err(
                    self.claim_id.to_string(),
                    "line_charge",
                    line.charge,
                ));
            }
        }

        let serv_line_total: f64 = self.service_lines.iter().map(|l| l.charge).sum();

        if !self.service_lines.is_empty()
            && (self.total_charge - serv_line_total).abs() > AMOUNT_TOLERANCE
        {
            validation_errors.push(ValidationError::ChargeMismatch {
                claim_id: (self.claim_id.to_string()),
                declared: (self.total_charge),
                computed: (serv_line_total),
            })
        }

        match validation_errors.is_empty() {
            true => Ok(()),
            false => Err(validation_errors),
        }
    }
}

fn negative_amount_err() -> impl Fn(String, &'static str, f64) -> ValidationError {
    |claim_id, field, value| ValidationError::NegativeAmount {
        claim_id,
        field,
        value,
    }
}

/// `NaN` counts as negative, because it isn't a valid amount either.
fn is_neg_amount(amount: f64) -> bool {
    amount < 0.0 || amount.is_nan()
}

/// Rules:
/// 1. `status_code` is in [`CLAIM_STATUS_CODES`] → else `UnknownStatus`.
/// 2. `total_charge` and `paid` ≥ 0 → else `NegativeAmount`
///    (`field`: `"total_charge"` or `"paid"`).
/// 3. If there are service lines (835s may omit them):
///    - `total_charge` equals the sum of line charges → else `ChargeMismatch`;
///    - `paid` equals the sum of line payments → else `PaymentMismatch`.
impl Validate for Remit<'_> {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut validation_errors = Vec::new();
        if !CLAIM_STATUS_CODES.contains(&self.status_code) {
            validation_errors.push(ValidationError::UnknownStatus {
                claim_id: (self.claim_id.to_string()),
                code: (self.status_code.to_string()),
            })
        }

        if is_neg_amount(self.total_charge) {
            validation_errors.push(negative_amount_err()(
                self.claim_id.to_string(),
                "total_charge",
                self.total_charge,
            ));
        }

        if is_neg_amount(self.paid) {
            validation_errors.push(negative_amount_err()(
                self.claim_id.to_string(),
                "paid",
                self.paid,
            ));
        }

        if !self.service_lines.is_empty() {
            let line_charge: f64 = self.service_lines.iter().map(|l| l.charge).sum();
            let line_paid: f64 = self.service_lines.iter().filter_map(|l| l.paid).sum();

            if (self.total_charge - line_charge).abs() > AMOUNT_TOLERANCE {
                validation_errors.push(ValidationError::ChargeMismatch {
                    claim_id: self.claim_id.to_string(),
                    declared: self.total_charge,
                    computed: line_charge,
                });
            }

            if (self.paid - line_paid).abs() > AMOUNT_TOLERANCE {
                validation_errors.push(ValidationError::PaymentMismatch {
                    claim_id: self.claim_id.to_string(),
                    declared: self.paid,
                    computed: line_paid,
                });
            }
        }

        into_result(validation_errors)
    }
}

/// Every remit's errors, plus `RemittanceTotalMismatch` if `total_payment`
/// isn't the sum of the remits' `paid`.
impl Validate for Remittance<'_> {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = validate_all(&self.remits).err().unwrap_or_default();

        let paid_total: f64 = self.remits.iter().map(|r| r.paid).sum();
        if (self.total_payment - paid_total).abs() > AMOUNT_TOLERANCE {
            errors.push(ValidationError::RemittanceTotalMismatch {
                declared: self.total_payment,
                computed: paid_total,
            });
        }

        into_result(errors)
    }
}

/// Delegate to the claims (via [`validate_all`]) or the remittance.
impl Validate for Transaction<'_> {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        match self {
            Transaction::Claims(set) => validate_all(&set.claims),
            Transaction::Remittance(r) => r.validate(),
        }
    }
}

/// Every transaction's errors, concatenated in order.
impl Validate for Interchange<'_> {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        validate_all(&self.transactions)
    }
}

/// Validate a slice of anything `Validate`, concatenating all errors.
/// Generic, so it works for `&[Claim]`, `&[Remit]`, `&[Transaction]`, ...
pub fn validate_all<T: Validate>(items: &[T]) -> Result<(), Vec<ValidationError>> {
    let errors: Vec<ValidationError> = items
        .iter()
        .filter_map(|item| item.validate().err())
        .flatten()
        .collect();

    into_result(errors)
}

/// Empty list → `Ok`, otherwise `Err` with the list.
fn into_result(errors: Vec<ValidationError>) -> Result<(), Vec<ValidationError>> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
