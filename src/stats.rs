//! Task 7: aggregate totals with iterator chains.
//!
//! `Summary` already derives `Serialize` so `main.rs` can print it; the serde
//! work for you is in `transaction.rs` (Task 5).

use std::cmp::Reverse;
use std::collections::BTreeMap;

use serde::Serialize;

use crate::error::EdiError;
use crate::transaction::Interchange;
use crate::validate::Validate;
use crate::Transaction;

/// Totals for one file, or for many files merged together.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Summary {
    /// Files that parsed successfully.
    pub files_processed: usize,
    /// Files that failed to read or parse.
    pub files_failed: usize,
    /// One error message per failed file (each names its file), sorted.
    pub failed_files: Vec<String>,
    /// Transaction sets (`ST`..`SE`) across all files.
    pub transactions: usize,
    /// 837 claims (`CLM`).
    pub claims: usize,
    /// 835 claim payments (`CLP`).
    pub remits: usize,
    /// Service lines of both kinds (`SV1` + `SVC`).
    pub service_lines: usize,
    /// Sum of 837 claim `total_charge` (CLM02).
    pub total_billed: f64,
    /// Sum of 835 remit `paid` (CLP04).
    pub total_paid: f64,
    /// Number of `ValidationError`s found.
    pub validation_errors: usize,
    /// How many service lines (837 and 835) used each procedure code.
    /// A `BTreeMap` so the JSON output is in a stable order.
    pub procedure_counts: BTreeMap<String, usize>,
}

impl Summary {
    /// Summarize one successfully parsed file (so `files_processed` is 1 and
    /// `files_failed` is 0). Runs validation to fill in `validation_errors`.
    ///
    /// Try to write each field as a single iterator chain, without `for` loops
    /// or `mut` counters.
    pub fn from_interchange(interchange: &Interchange<'_>) -> Summary {
        let txns = || interchange.transactions.iter();
        let claims = || txns().flat_map(Transaction::claims);
        let claim_lines = || claims().flat_map(|clm| &clm.service_lines);
        let remits = || txns().flat_map(Transaction::remits);
        let remit_lines = || remits().flat_map(|rm| &rm.service_lines);

        let procedure_counts = claim_lines()
            .map(|ln| ln.procedure_code)
            .chain(remit_lines().map(|ln| ln.procedure_code))
            .fold(BTreeMap::new(), |mut map, cd| {
                *map.entry(cd.to_string()).or_default() += 1;
                map
            });

        Summary {
            files_processed: (1),
            files_failed: (0),
            failed_files: (Vec::new()),
            transactions: (txns().count()),
            claims: (claims().count()),
            remits: (remits().count()),
            service_lines: (claim_lines().count() + remit_lines().count()),
            total_billed: (claims().map(|clm| clm.total_charge).sum()),
            total_paid: (remits().map(|rm| rm.paid).sum()),
            validation_errors: interchange.validate().err().map_or(0, |errs| errs.len()),
            procedure_counts: (procedure_counts),
        }
    }

    /// Summary for one file that failed: `files_failed` is 1 and
    /// `failed_files` holds the error's message. Errors from `process_file`
    /// already name their file (`InFile`, `Io`), so the message includes the path.
    pub fn failure(error: &EdiError) -> Summary {
        Summary {
            files_processed: (0),
            files_failed: (1),
            failed_files: (vec![error.to_string()]),
            transactions: (0),
            claims: (0),
            remits: (0),
            service_lines: (0),
            total_billed: (0.0),
            total_paid: (0.0),
            validation_errors: (0),
            procedure_counts: (BTreeMap::new()),
        }
    }

    /// Combine two summaries: add every count and total, add procedure counts
    /// per code, concatenate `failed_files` and keep them sorted.
    ///
    /// `Summary::default()` must be an identity: `s.merge(Summary::default()) == s`.
    /// That plus associativity is what lets rayon `reduce` with it in Task 8.
    pub fn merge(self, other: Summary) -> Summary {
        let mut failed_files = [self.failed_files, other.failed_files].concat();
        failed_files.sort();

        let mut procedure_counts = self.procedure_counts;
        for (code, count) in other.procedure_counts {
            *procedure_counts.entry(code).or_default() += count;
        }

        Summary {
            files_processed: self.files_processed + other.files_processed,
            files_failed: self.files_failed + other.files_failed,
            failed_files,
            transactions: self.transactions + other.transactions,
            claims: self.claims + other.claims,
            remits: self.remits + other.remits,
            service_lines: self.service_lines + other.service_lines,
            total_billed: self.total_billed + other.total_billed,
            total_paid: self.total_paid + other.total_paid,
            validation_errors: self.validation_errors + other.validation_errors,
            procedure_counts,
        }
    }

    /// The `n` most used procedure codes, highest count first; ties broken by
    /// code, ascending. Borrows the codes from `self` instead of cloning them.
    pub fn top_procedures(&self, n: usize) -> Vec<(&str, usize)> {
        let mut counts: Vec<(&str, usize)> = self
            .procedure_counts
            .iter()
            .map(|(cd, &count)| (cd.as_str(), count))
            .collect();

        counts.sort_by_key(|&(code, count)| (Reverse(count), code)); // sort in place
        counts.truncate(n); // truncate in place

        counts
    }
}
