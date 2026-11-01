//! Process a directory of files, optionally in parallel with rayon.

use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::thread;

use crate::error::{EdiError, ValidationError};
use crate::stats::Summary;
use crate::transaction::Interchange;
use crate::validate::Validate;
use crate::validation_log::ValidationLog;
use crate::{parse_str, to_json};
use rayon::prelude::*;

/// File extensions (case-insensitive) that `collect_files` picks up.
pub const EXTENSIONS: &[&str] = &["edi", "x12"];

/// Reader threads used by a parallel run unless told otherwise. Concurrent
/// `open` calls get slower past about 4 threads on large directories.
pub const DEFAULT_READERS: NonZeroUsize = NonZeroUsize::new(4).unwrap();

/// Files read but not yet parsed. Bounds memory when reads outpace parsing.
const READ_QUEUE: usize = 256;

/// How [`process_dir`] gets through its files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// One file at a time on the calling thread.
    Sequential,
    /// `readers` threads read files for the rayon workers to parse.
    Parallel { readers: NonZeroUsize },
}

/// `true` is parallel with [`DEFAULT_READERS`], `false` is sequential.
impl From<bool> for Mode {
    fn from(parallel: bool) -> Self {
        if parallel {
            Mode::Parallel {
                readers: DEFAULT_READERS,
            }
        } else {
            Mode::Sequential
        }
    }
}

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

/// A file's [`Summary`], plus the validation errors that it counted.
type Checked = (Summary, Vec<ValidationError>);

fn check(interchange: &Interchange<'_>) -> Checked {
    let errors = interchange.validate().err().unwrap_or_default();
    (
        Summary::with_validation_errors(interchange, &errors),
        errors,
    )
}

/// Parse `src`, write it as JSON to `dest` (creating parent directories), and
/// return its [`Summary`].
pub fn parse_to_file(
    src: &Path,
    dest: &Path,
    pretty: bool,
) -> Result<Summary, Box<dyn std::error::Error>> {
    parse_and_check(src, dest, pretty).map(|(summary, _)| summary)
}

fn parse_and_check(
    src: &Path,
    dest: &Path,
    pretty: bool,
) -> Result<Checked, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(src)?;
    let interchange = parse_str(&contents)?;
    let json = to_json(&interchange, pretty)?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, json + "\n")?;
    Ok(check(&interchange))
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
/// to `out` as `.json`, mirroring the layout of `dir`. Validation errors go to
/// `<out>/validation_errors.json` (see [`ValidationLog`]).
///
/// A file that fails is recorded in the report and gets no output file; it
/// does not abort the run. Only failing to list `dir`, or to write the
/// validation error file, is an `Err`.
pub fn parse_dir(dir: &Path, out: &Path, pretty: bool) -> Result<ParseDirReport, EdiError> {
    let files = collect_files_recursive(dir)?;
    let log = ValidationLog::create(out)?;

    let summary = files
        .par_iter()
        .map(|src| {
            let relative = src.strip_prefix(dir).unwrap_or(src);
            let dest = out.join(relative).with_extension("json");
            match parse_and_check(src, &dest, pretty) {
                Ok((summary, errors)) => log.write(src, &errors).map(|()| summary),
                Err(e) => Ok(Summary {
                    files_failed: 1,
                    failed_files: vec![format!("{}: {e}", src.display())],
                    ..Summary::default()
                }),
            }
        })
        .try_reduce(Summary::default, |total, next| Ok(total.merge(next)))?;

    log.finish()?;

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
    check_file(path).map(|(summary, _)| summary)
}

fn check_file(path: &Path) -> Result<Checked, EdiError> {
    check_contents(path, fs::read_to_string(path))
}

fn check_contents(path: &Path, contents: io::Result<String>) -> Result<Checked, EdiError> {
    let contents = contents.map_err(|e| EdiError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let interchange = parse_str(contents.as_str()).map_err(|e| EdiError::InFile {
        path: (path.to_path_buf()),
        source: (Box::new(e)),
    })?;

    Ok(check(&interchange))
}

/// Summarize every file in `dir`, merging into one [`Summary`].
///
/// A file that fails becomes [`Summary::failure`]; it does not abort the run.
/// Only failing to list the directory is an `Err`.
///
/// `mode` is a [`Mode`], or a `bool` where `true` means parallel. All modes
/// must produce the same counts.
pub fn process_dir(dir: &Path, mode: impl Into<Mode>) -> Result<Summary, EdiError> {
    summarize_files(&collect_files(dir)?, mode.into(), None)
}

/// [`process_dir`], also writing validation errors to
/// `<out>/validation_errors.json` (see [`ValidationLog`]).
///
/// Failing to write that file is an `Err`, as well as failing to list `dir`.
pub fn process_dir_with_log(
    dir: &Path,
    mode: impl Into<Mode>,
    out: &Path,
) -> Result<Summary, EdiError> {
    let files = collect_files(dir)?;
    let log = ValidationLog::create(out)?;
    let summary = summarize_files(&files, mode.into(), Some(&log))?;
    log.finish()?;
    Ok(summary)
}

fn summarize_files(
    files: &[PathBuf],
    mode: Mode,
    log: Option<&ValidationLog>,
) -> Result<Summary, EdiError> {
    let summarize = |f: &PathBuf, contents| match check_contents(f, contents) {
        Ok((summary, errors)) => match log {
            Some(log) => log.write(f, &errors).map(|()| summary),
            None => Ok(summary),
        },
        Err(e) => Ok(Summary::failure(&e)),
    };

    match mode {
        Mode::Sequential => files
            .iter()
            .map(|f| summarize(f, fs::read_to_string(f)))
            .try_fold(Summary::default(), |total, next| Ok(total.merge(next?))),
        Mode::Parallel { readers } => read_then_summarize(files, readers, summarize),
    }
}

/// Read `files` on `readers` threads and pass each result to `summarize` on the
/// rayon workers, merging what comes back.
///
/// Files arrive in whatever order they were read, so `summarize` must not
/// depend on order. Reads are kept off the workers so that they can't all block
/// in `open`. If `summarize` returns an `Err`, the receiver is dropped, which
/// stops the readers.
fn read_then_summarize<'f>(
    files: &'f [PathBuf],
    readers: NonZeroUsize,
    summarize: impl Fn(&'f PathBuf, io::Result<String>) -> Result<Summary, EdiError> + Sync,
) -> Result<Summary, EdiError> {
    let next = AtomicUsize::new(0);

    thread::scope(|scope| {
        let (sender, receiver) = sync_channel(READ_QUEUE);
        for _ in 0..readers.get() {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || {
                while let Some(file) = files.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if sender.send((file, fs::read_to_string(file))).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);

        receiver
            .into_iter()
            .par_bridge()
            .map(|(file, contents)| summarize(file, contents))
            .try_reduce(Summary::default, |total, next| Ok(total.merge(next)))
    })
}
