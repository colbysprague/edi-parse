//! Writes every validation error from a directory run to a JSON array.
//!
//! Errors go to disk as each file finishes, rather than being held in
//! [`Summary`](crate::Summary), so memory use doesn't grow with the error count.
//! The array is only closed by [`ValidationLog::finish`], so a run that stops
//! early leaves a file that isn't valid JSON.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use crate::error::{EdiError, ValidationError};

/// The name of the file written to the output directory.
pub const VALIDATION_LOG_FILE: &str = "validation_errors.json";

/// One entry in the array.
#[derive(Serialize)]
struct Record<'a> {
    file: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    claim_id: Option<&'a str>,
    #[serde(rename = "type")]
    kind: &'static str,
    message: String,
}

/// A validation error file that parallel workers can share.
///
/// Each file's entries are written together, so they can't interleave with
/// another file's, but files appear in whatever order they finish.
#[derive(Debug)]
pub struct ValidationLog {
    path: PathBuf,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    writer: BufWriter<File>,
    empty: bool,
}

impl ValidationLog {
    /// Create `<out>/validation_errors.json`, creating `out` if needed. An
    /// existing file is emptied, so errors from an earlier run can't remain.
    pub fn create(out: &Path) -> Result<Self, EdiError> {
        let path = out.join(VALIDATION_LOG_FILE);
        let writer = fs::create_dir_all(out).and_then(|()| {
            let mut writer = BufWriter::new(File::create(&path)?);
            writer.write_all(b"[")?;
            Ok(writer)
        });

        match writer {
            Ok(writer) => Ok(ValidationLog {
                path,
                state: Mutex::new(State {
                    writer,
                    empty: true,
                }),
            }),
            Err(source) => Err(EdiError::Write { path, source }),
        }
    }

    /// Add one entry per error, each naming `file`.
    pub fn write(&self, file: &Path, errors: &[ValidationError]) -> Result<(), EdiError> {
        if errors.is_empty() {
            return Ok(());
        }

        self.write_entries(file, errors)
            .map_err(|source| EdiError::Write {
                path: self.path.clone(),
                source,
            })
    }

    fn write_entries(&self, file: &Path, errors: &[ValidationError]) -> io::Result<()> {
        let file = file.to_string_lossy();
        let entries = errors
            .iter()
            .map(|error| entry(&file, error))
            .collect::<serde_json::Result<Vec<_>>>()?
            .join(",\n");

        let mut state = self.state.lock().unwrap();
        let separator = if state.empty { "\n" } else { ",\n" };
        state.empty = false;
        state.writer.write_all(separator.as_bytes())?;
        state.writer.write_all(entries.as_bytes())
    }

    /// Close the array and flush the file. A write error here would otherwise
    /// be lost on drop.
    pub fn finish(self) -> Result<(), EdiError> {
        let ValidationLog { path, state } = self;
        let mut state = state.into_inner().unwrap();
        let close = if state.empty { "]\n" } else { "\n]\n" };

        state
            .writer
            .write_all(close.as_bytes())
            .and_then(|()| state.writer.flush())
            .map_err(|source| EdiError::Write { path, source })
    }
}

/// One error as pretty JSON, indented to sit inside the array.
fn entry(file: &str, error: &ValidationError) -> serde_json::Result<String> {
    let json = serde_json::to_string_pretty(&record(file, error))?;
    Ok(format!("  {}", json.replace('\n', "\n  ")))
}

fn record<'a>(file: &'a str, error: &'a ValidationError) -> Record<'a> {
    let claim_id = error.claim_id();
    let text = error.to_string();

    // `claim_id` has its own field, so don't repeat it in the message.
    let message = claim_id
        .and_then(|id| text.strip_prefix(&format!("claim {id}: ")))
        .map_or_else(|| text.clone(), String::from);

    Record {
        file,
        claim_id,
        kind: error.kind(),
        message,
    }
}
