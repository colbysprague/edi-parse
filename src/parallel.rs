//! Task 8: process a directory of files, optionally in parallel with rayon.

use std::path::{Path, PathBuf};

use crate::error::EdiError;
use crate::stats::Summary;

/// File extensions (case-insensitive) that `collect_files` picks up.
pub const EXTENSIONS: &[&str] = &["edi", "x12"];

/// List the EDI files directly inside `dir` (not recursive), sorted by path.
/// Files whose extension isn't in [`EXTENSIONS`] are skipped.
///
/// Errors with [`EdiError::Io`] if the directory can't be read.
pub fn collect_files(dir: &Path) -> Result<Vec<PathBuf>, EdiError> {
    todo!("Task 8")
}

/// Read, parse, and summarize one file.
///
/// The file contents are a `String` local to this function, and the parsed
/// `Interchange` borrows from it, so the `Interchange` can't be returned.
/// That's why this returns an owned `Summary` instead.
///
/// Any error is wrapped as [`EdiError::InFile`] (or is an [`EdiError::Io`]
/// that already carries the path).
pub fn process_file(path: &Path) -> Result<Summary, EdiError> {
    todo!("Task 8")
}

/// Summarize every file in `dir`, merging into one [`Summary`].
///
/// A file that fails becomes [`Summary::failure`]; it does not abort the run.
/// Only failing to list the directory is an `Err`.
///
/// `parallel == true` uses rayon; `false` uses a plain iterator. Both must
/// produce the same counts.
pub fn process_dir(dir: &Path, parallel: bool) -> Result<Summary, EdiError> {
    todo!("Task 8: rayon par_iter + reduce")
}
