//! Task 8: process a directory of files, optionally in parallel with rayon.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::EdiError;
use crate::parse_str;
use crate::stats::Summary;
use rayon::prelude::*;

/// File extensions (case-insensitive) that `collect_files` picks up.
pub const EXTENSIONS: &[&str] = &["edi", "x12"];

/// List the EDI files directly inside `dir` (not recursive), sorted by path.
/// Files whose extension isn't in [`EXTENSIONS`] are skipped.
///
/// Errors with [`EdiError::Io`] if the directory can't be read.
pub fn collect_files(dir: &Path) -> Result<Vec<PathBuf>, EdiError> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| EdiError::Io {
        path: (dir.to_path_buf()),
        source: (e),
    })? {
        let entry = entry.map_err(|e| EdiError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;

        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if path.is_file() && EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
            out.push(path);
        }
    }

    out.sort();
    Ok(out)
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
    let contents = fs::read_to_string(path).map_err(|e| EdiError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let interchange = parse_str(contents.as_str()).map_err(|e| EdiError::InFile {
        path: (path.to_path_buf()),
        source: (Box::new(e)),
    })?;

    Ok(Summary::from_interchange(&interchange))
}

/// Summarize every file in `dir`, merging into one [`Summary`].
///
/// A file that fails becomes [`Summary::failure`]; it does not abort the run.
/// Only failing to list the directory is an `Err`.
///
/// `parallel == true` uses rayon; `false` uses a plain iterator. Both must
/// produce the same counts.
pub fn process_dir(dir: &Path, parallel: bool) -> Result<Summary, EdiError> {
    let files = collect_files(dir)?;

    let summarize = |f: &PathBuf| match process_file(f) {
        Ok(s) => s,
        Err(e) => Summary::failure(&e),
    };

    let total = if parallel {
        files
            .par_iter()
            .map(summarize)
            .reduce(Summary::default, Summary::merge)
    } else {
        files
            .iter()
            .map(summarize)
            .fold(Summary::default(), Summary::merge)
    };

    Ok(total)
}
