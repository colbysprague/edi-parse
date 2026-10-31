//! Task 8: process a directory of files, optionally in parallel with rayon.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::EdiError;
use crate::stats::Summary;
use crate::{parse_str, to_json};
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

/// Like [`collect_files`], but also descends into subdirectories.
pub fn collect_files_recursive(dir: &Path) -> Result<Vec<PathBuf>, EdiError> {
    let io_err = |e| EdiError::Io {
        path: dir.to_path_buf(),
        source: e,
    };

    let mut out = collect_files(dir)?;
    for entry in fs::read_dir(dir).map_err(io_err)? {
        let path = entry.map_err(io_err)?.path();
        if path.is_dir() {
            out.extend(collect_files_recursive(&path)?);
        }
    }

    out.sort();
    Ok(out)
}

/// Result of [`parse_dir`].
#[derive(Debug, Clone, PartialEq)]
pub struct ParseDirReport {
    /// How many JSON files were written.
    pub written: usize,
    /// One `path: error` message per file that failed, sorted by path.
    pub failed: Vec<String>,
    /// Totals across every file, merged.
    pub summary: Summary,
}

/// Parse `src`, write it as JSON to `dest` (creating parent directories), and
/// return its [`Summary`].
pub fn parse_to_file(
    src: &Path,
    dest: &Path,
    pretty: bool,
) -> Result<Summary, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(src)?;
    let interchange = parse_str(&contents)?;
    let json = to_json(&interchange, pretty)?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, json + "\n")?;
    Ok(Summary::from_interchange(&interchange))
}

/// Write `summary` as pretty JSON to `<out>/summary.json`, creating `out` if
/// needed, and return the path written.
pub fn write_summary(summary: &Summary, out: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(summary)?;
    fs::create_dir_all(out)?;
    let dest = out.join("summary.json");
    fs::write(&dest, json + "\n")?;
    Ok(dest)
}

/// Parse every file under `dir` (recursively, in parallel) and write each one
/// to `out` as `.json`, mirroring the layout of `dir`.
///
/// A file that fails is recorded in the report and gets no output file; it
/// does not abort the run. Only failing to list `dir` is an `Err`.
pub fn parse_dir(dir: &Path, out: &Path, pretty: bool) -> Result<ParseDirReport, EdiError> {
    let files = collect_files_recursive(dir)?;

    let summary = files
        .par_iter()
        .map(|src| {
            let relative = src.strip_prefix(dir).unwrap_or(src);
            let dest = out.join(relative).with_extension("json");
            parse_to_file(src, &dest, pretty).unwrap_or_else(|e| Summary {
                files_failed: 1,
                failed_files: vec![format!("{}: {e}", src.display())],
                ..Summary::default()
            })
        })
        .reduce(Summary::default, Summary::merge);

    Ok(ParseDirReport {
        written: summary.files_processed,
        failed: summary.failed_files.clone(),
        summary,
    })
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
