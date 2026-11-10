//! Browser bindings for `edi-parse`.
//!
//! The functions take and return JSON strings so the JS side needs no glue
//! beyond `JSON.parse`.

use edi_parse::{parse_str, to_json, Summary, Validate, ValidationError};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// One validation error, in the same shape as `validation_errors.json`.
#[derive(Serialize)]
struct Record<'a> {
    file: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    claim_id: Option<&'a str>,
    #[serde(rename = "type")]
    kind: &'static str,
    message: String,
}

fn records<'a>(file: &'a str, errors: &'a [ValidationError]) -> impl Iterator<Item = Record<'a>> {
    errors.iter().map(move |e| Record {
        file,
        claim_id: e.claim_id(),
        kind: e.kind(),
        message: e.to_string(),
    })
}

#[derive(Deserialize)]
struct Input {
    path: String,
    contents: String,
}

#[derive(Serialize)]
struct FileResult<'a> {
    path: &'a str,
    /// Parsed interchange, as `edi-parse parse --pretty` would write it.
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    /// Parse error, if the file didn't parse.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct RunResult<'a> {
    files: Vec<FileResult<'a>>,
    /// `summary.json`, pretty-printed.
    summary: String,
    /// `validation_errors.json`, pretty-printed.
    validation_errors: String,
}

/// Parse a JSON array of `{path, contents}` and return
/// `{files, summary, validation_errors}`, mirroring `edi-parse parse <dir> --out`.
/// The JSON documents inside are pre-rendered strings, so numbers keep Rust's
/// formatting (`100.0`, not `100`).
#[wasm_bindgen]
pub fn parse_files(inputs: &str) -> Result<String, JsError> {
    let inputs: Vec<Input> = serde_json::from_str(inputs)?;
    let mut files = Vec::with_capacity(inputs.len());
    let mut summary = Summary::default();
    let mut errors_per_file: Vec<(&str, Vec<ValidationError>)> = Vec::new();

    for input in &inputs {
        match parse_str(&input.contents) {
            Ok(interchange) => {
                let errors = interchange.validate().err().unwrap_or_default();
                summary = summary.merge(Summary::with_validation_errors(&interchange, &errors));
                files.push(FileResult {
                    path: &input.path,
                    output: Some(to_json(&interchange, true)?),
                    error: None,
                });
                errors_per_file.push((&input.path, errors));
            }
            Err(e) => {
                summary = summary.merge(Summary {
                    files_failed: 1,
                    failed_files: vec![format!("{}: {e}", input.path)],
                    ..Summary::default()
                });
                files.push(FileResult {
                    path: &input.path,
                    output: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let validation_errors: Vec<Record> = errors_per_file
        .iter()
        .flat_map(|(path, errors)| records(path, errors))
        .collect();

    Ok(serde_json::to_string(&RunResult {
        files,
        summary: serde_json::to_string_pretty(&summary)?,
        validation_errors: serde_json::to_string_pretty(&validation_errors)?,
    })?)
}
