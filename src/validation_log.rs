//! Writes every validation error from a directory run to a JSON Lines file.
//!
//! Errors go to disk as each file finishes, rather than being held in
//! [`Summary`](crate::Summary), so memory use doesn't grow with the error count.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use crate::error::{EdiError, ValidationError};

/// The name of the file written to the output directory.
pub const VALIDATION_LOG_FILE: &str = "validation_errors.jsonl";

/// One line of the file.
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
/// Each file's lines are written together, so they can't interleave with
/// another file's, but files appear in whatever order they finish.
#[derive(Debug)]
pub struct ValidationLog {
    path: PathBuf,
    writer: Mutex<BufWriter<File>>,
}

impl ValidationLog {
    /// Create `<out>/validation_errors.jsonl`, creating `out` if needed. An
    /// existing file is emptied, so errors from an earlier run can't remain.
    pub fn create(out: &Path) -> Result<Self, EdiError> {
        let path = out.join(VALIDATION_LOG_FILE);
        let file = fs::create_dir_all(out).and_then(|()| File::create(&path));

        match file {
            Ok(file) => Ok(ValidationLog {
                path,
                writer: Mutex::new(BufWriter::new(file)),
            }),
            Err(source) => Err(EdiError::Write { path, source }),
        }
    }

    /// Add one line per error, each naming `file`.
    pub fn write(&self, file: &Path, errors: &[ValidationError]) -> Result<(), EdiError> {
        if errors.is_empty() {
            return Ok(());
        }

        self.write_lines(file, errors)
            .map_err(|source| EdiError::Write {
                path: self.path.clone(),
                source,
            })
    }

    fn write_lines(&self, file: &Path, errors: &[ValidationError]) -> io::Result<()> {
        let file = file.to_string_lossy();
        let mut lines = Vec::new();

        for error in errors {
            serde_json::to_writer(&mut lines, &record(&file, error))?;
            lines.push(b'\n');
        }

        self.writer.lock().unwrap().write_all(&lines)
    }

    /// Flush the file. A write error here would otherwise be lost on drop.
    pub fn finish(self) -> Result<(), EdiError> {
        let ValidationLog { path, writer } = self;

        writer
            .into_inner()
            .unwrap()
            .flush()
            .map_err(|source| EdiError::Write { path, source })
    }
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
