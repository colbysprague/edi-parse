//! Task 8: `collect_files`, `process_file`, and `process_dir` with rayon.

mod common;

use std::path::Path;

use common::*;
use edi_parse::{
    collect_files, parse_dir, process_dir, process_dir_with_log, process_file, write_summary,
    EdiError, Summary, VALIDATION_LOG_FILE,
};
use serde_json::{json, Value};

/// Parallel float sums can differ from sequential ones in the last bits,
/// so compare totals approximately and everything else exactly.
#[track_caller]
fn assert_same(a: &Summary, b: &Summary) {
    assert_eq!(a.files_processed, b.files_processed);
    assert_eq!(a.files_failed, b.files_failed);
    assert_eq!(a.failed_files, b.failed_files);
    assert_eq!(a.transactions, b.transactions);
    assert_eq!(a.claims, b.claims);
    assert_eq!(a.remits, b.remits);
    assert_eq!(a.service_lines, b.service_lines);
    assert_eq!(a.validation_errors, b.validation_errors);
    assert_eq!(a.validation_error_counts, b.validation_error_counts);
    assert_eq!(
        a.files_with_validation_errors,
        b.files_with_validation_errors
    );
    assert_eq!(a.procedure_counts, b.procedure_counts);
    assert!((a.total_billed - b.total_billed).abs() < 1e-6);
    assert!((a.total_paid - b.total_paid).abs() < 1e-6);
}

#[test]
fn collect_files_is_sorted_and_filtered() {
    let dir = temp_dir("collect");
    for name in [
        "b.edi",
        "a.X12",
        "c.edi",
        "notes.txt",
        "README",
        "d.edi.bak",
    ] {
        std::fs::write(dir.join(name), "x").unwrap();
    }
    std::fs::create_dir(dir.join("sub.edi")).unwrap(); // a directory, not a file
    std::fs::write(dir.join("sub.edi").join("nested.edi"), "x").unwrap();

    let names: Vec<String> = collect_files(&dir)
        .unwrap()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["a.X12", "b.edi", "c.edi"]);
}

#[test]
fn collect_files_on_missing_dir() {
    let err = collect_files(Path::new("/definitely/not/a/real/dir")).unwrap_err();
    assert!(matches!(err, EdiError::Io { .. }), "got {err:?}");
}

#[test]
fn process_file_ok() {
    let s = process_file(&samples_dir().join(S_837_MULTI)).unwrap();
    assert_eq!(s.files_processed, 1);
    assert_eq!(s.claims, 3);
}

#[test]
fn process_file_errors_mention_the_path() {
    let path = invalid_dir().join("missing_iea.edi");
    let err = process_file(&path).unwrap_err();
    match &err {
        EdiError::InFile { path: p, source } => {
            assert_eq!(p, &path);
            assert!(matches!(**source, EdiError::MissingIea), "got {source:?}");
        }
        other => panic!("expected InFile, got {other:?}"),
    }
    assert!(err.to_string().contains("missing_iea.edi"));
}

#[test]
fn process_file_missing_file() {
    let err = process_file(Path::new("/no/such/file.edi")).unwrap_err();
    let is_io = match &err {
        EdiError::Io { .. } => true,
        EdiError::InFile { source, .. } => matches!(**source, EdiError::Io { .. }),
        _ => false,
    };
    assert!(is_io, "got {err:?}");
}

#[test]
fn samples_sequential() {
    let s = process_dir(&samples_dir(), false).unwrap();
    assert_eq!(s.files_processed, 4);
    assert_eq!(s.files_failed, 0);
    assert_eq!(s.claims, 5);
    assert_eq!(s.remits, 5);
    assert_eq!(s.service_lines, 16);
    assert_approx(s.total_billed, 635.75);
    assert_approx(s.total_paid, 405.0);
}

#[test]
fn samples_parallel_matches_sequential() {
    let seq = process_dir(&samples_dir(), false).unwrap();
    let par = process_dir(&samples_dir(), true).unwrap();
    assert_same(&seq, &par);
}

#[test]
fn invalid_files_are_counted_not_fatal() {
    for parallel in [false, true] {
        let s = process_dir(&invalid_dir(), parallel).unwrap();
        assert_eq!(s.files_processed, 0);
        assert_eq!(s.files_failed, 4);
        for name in [
            "truncated_isa",
            "se_count_mismatch",
            "clp_bad_amount",
            "missing_iea",
        ] {
            assert!(
                s.failed_files.iter().any(|f| f.contains(name)),
                "{name} not in {:?}",
                s.failed_files
            );
        }
        let mut sorted = s.failed_files.clone();
        sorted.sort();
        assert_eq!(s.failed_files, sorted, "failed_files should be sorted");
    }
}

#[test]
fn mixed_directory() {
    let dir = temp_dir("mixed");
    for name in ALL_SAMPLES {
        std::fs::copy(samples_dir().join(name), dir.join(name)).unwrap();
    }
    for name in ["missing_iea.edi", "clp_bad_amount.edi"] {
        std::fs::copy(invalid_dir().join(name), dir.join(name)).unwrap();
    }
    std::fs::write(dir.join("ignore-me.txt"), "not edi").unwrap();

    let seq = process_dir(&dir, false).unwrap();
    let par = process_dir(&dir, true).unwrap();
    assert_eq!(seq.files_processed, 4);
    assert_eq!(seq.files_failed, 2);
    assert_same(&seq, &par);
}

#[test]
fn many_files_parallel_matches_sequential() {
    let dir = temp_dir("many");
    for i in 0..50 {
        for name in ALL_SAMPLES {
            std::fs::copy(samples_dir().join(name), dir.join(format!("{i:03}_{name}"))).unwrap();
        }
    }
    let seq = process_dir(&dir, false).unwrap();
    let par = process_dir(&dir, true).unwrap();
    assert_eq!(seq.files_processed, 200);
    assert_eq!(seq.claims, 250);
    assert_eq!(seq.procedure_counts["99213"], 250);
    assert_approx(seq.total_paid, 405.0 * 50.0);
    assert_same(&seq, &par);
}

#[test]
fn empty_directory() {
    let dir = temp_dir("empty");
    for parallel in [false, true] {
        assert_eq!(process_dir(&dir, parallel).unwrap(), Summary::default());
    }
}

#[test]
fn process_dir_on_missing_dir() {
    assert!(matches!(
        process_dir(Path::new("/definitely/not/a/real/dir"), true),
        Err(EdiError::Io { .. })
    ));
}

#[test]
fn parse_dir_mirrors_layout_and_continues_past_failures() {
    let dir = temp_dir("parse-dir-in");
    let out = temp_dir("parse-dir-out");
    std::fs::create_dir(dir.join("nested")).unwrap();
    std::fs::copy(
        samples_dir().join("835_multi_clp.edi"),
        dir.join("nested").join("ok.edi"),
    )
    .unwrap();
    std::fs::write(dir.join("bad.edi"), "x").unwrap();
    std::fs::write(dir.join("notes.txt"), "x").unwrap();

    let report = parse_dir(&dir, &out, false).unwrap();

    assert_eq!(report.written, 1);
    assert_eq!(report.failed.len(), 1);
    assert!(report.failed[0].contains("bad.edi"), "{:?}", report.failed);
    assert!(out.join("nested").join("ok.json").is_file());
    assert!(!out.join("bad.json").exists());
    assert!(!out.join("notes.json").exists());
}

#[test]
fn parse_dir_summary_covers_nested_and_failed_files() {
    let dir = temp_dir("parse-dir-summary-in");
    let out = temp_dir("parse-dir-summary-out");
    std::fs::create_dir(dir.join("nested")).unwrap();
    std::fs::copy(
        samples_dir().join("835_multi_clp.edi"),
        dir.join("nested").join("ok.edi"),
    )
    .unwrap();
    std::fs::write(dir.join("bad.edi"), "x").unwrap();

    let report = parse_dir(&dir, &out, false).unwrap();
    let path = write_summary(&report.summary, &out).unwrap();

    assert_eq!(path, out.join("summary.json"));
    assert_eq!(report.summary.files_processed, 1);
    assert_eq!(report.summary.files_failed, 1);
    assert!(report.summary.remits > 0);

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written["files_processed"], 1);
    assert_eq!(written["files_failed"], 1);
}

#[test]
fn parse_dir_on_missing_dir() {
    let out = temp_dir("parse-dir-missing");
    assert!(matches!(
        parse_dir(Path::new("/definitely/not/a/real/dir"), &out, false),
        Err(EdiError::Io { .. })
    ));
}

/// One claim with a charge mismatch, and one with no service lines.
fn two_bad_claims() -> String {
    envelope(
        "HC",
        &[
            "ST*837*0001",
            "CLM*C1*90.00",
            "SV1*HC:99213*80.00",
            "CLM*C2*10.00",
            "SE*5*0001",
        ],
    )
}

fn remittance_total_mismatch() -> String {
    envelope(
        "HP",
        &[
            "ST*835*0001",
            "BPR*I*5.00",
            "CLP*C1*1*10.00*10.00",
            "SVC*HC:99213*10.00*10.00",
            "SE*5*0001",
        ],
    )
}

fn log_lines(out: &Path) -> Vec<Value> {
    let mut lines: Vec<Value> = std::fs::read_to_string(out.join(VALIDATION_LOG_FILE))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    lines.sort_by_key(Value::to_string);
    lines
}

#[test]
fn parse_dir_writes_each_validation_error_with_its_file() {
    let dir = temp_dir("log-input");
    let out = temp_dir("log-output");
    std::fs::create_dir(dir.join("nested")).unwrap();
    let src = dir.join("nested").join("bad.edi");
    std::fs::write(&src, two_bad_claims()).unwrap();

    let report = parse_dir(&dir, &out, false).unwrap();
    let file = src.to_string_lossy();
    assert_eq!(
        log_lines(&out),
        [
            json!({
                "file": file,
                "claim_id": "C1",
                "type": "charge_mismatch",
                "message": "declared charge 90.00 != service line total 80.00",
            }),
            json!({
                "file": file,
                "claim_id": "C2",
                "type": "no_service_lines",
                "message": "has no service lines",
            }),
        ]
    );
    assert_eq!(report.summary.validation_errors, 2);
    assert_eq!(report.summary.files_with_validation_errors, 1);
}

#[test]
fn error_with_no_claim_leaves_out_claim_id() {
    let dir = temp_dir("log-no-claim-input");
    let out = temp_dir("log-no-claim-output");
    std::fs::write(dir.join("bad.edi"), remittance_total_mismatch()).unwrap();

    parse_dir(&dir, &out, false).unwrap();
    let lines = log_lines(&out);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["type"], "remittance_total_mismatch");
    assert!(lines[0].get("claim_id").is_none());
}

#[test]
fn parallel_and_sequential_logs_have_the_same_lines() {
    let dir = temp_dir("log-modes-input");
    for i in 0..50 {
        std::fs::write(dir.join(format!("{i}.edi")), two_bad_claims()).unwrap();
    }
    std::fs::write(dir.join("clean.edi"), sample(S_837_MULTI)).unwrap();

    let parallel_out = temp_dir("log-modes-parallel");
    let sequential_out = temp_dir("log-modes-sequential");
    let parallel = process_dir_with_log(&dir, true, &parallel_out).unwrap();
    let sequential = process_dir_with_log(&dir, false, &sequential_out).unwrap();

    assert_same(&parallel, &sequential);
    assert_eq!(parallel.validation_errors, 100);
    assert_eq!(parallel.files_with_validation_errors, 50);
    assert_eq!(log_lines(&parallel_out).len(), 100);
    assert_eq!(log_lines(&parallel_out), log_lines(&sequential_out));
}

#[test]
fn clean_run_replaces_an_earlier_log_with_an_empty_one() {
    let dir = temp_dir("log-clean-input");
    let out = temp_dir("log-clean-output");
    let src = dir.join("file.edi");

    std::fs::write(&src, two_bad_claims()).unwrap();
    process_dir_with_log(&dir, true, &out).unwrap();
    assert_eq!(log_lines(&out).len(), 2);

    std::fs::write(&src, sample(S_837_MULTI)).unwrap();
    process_dir_with_log(&dir, true, &out).unwrap();
    assert!(log_lines(&out).is_empty());

    std::fs::write(&src, two_bad_claims()).unwrap();
    parse_dir(&dir, &out, false).unwrap();
    assert_eq!(log_lines(&out).len(), 2);

    std::fs::write(&src, sample(S_837_MULTI)).unwrap();
    parse_dir(&dir, &out, false).unwrap();
    assert!(log_lines(&out).is_empty());
}

#[test]
fn missing_input_dir_keeps_the_earlier_log() {
    let out = temp_dir("log-missing-input");
    std::fs::write(out.join(VALIDATION_LOG_FILE), "earlier\n").unwrap();

    let missing = Path::new("/definitely/not/a/real/dir");
    assert!(process_dir_with_log(missing, true, &out).is_err());
    assert!(parse_dir(missing, &out, false).is_err());
    assert_eq!(
        std::fs::read_to_string(out.join(VALIDATION_LOG_FILE)).unwrap(),
        "earlier\n"
    );
}

#[test]
fn failing_to_write_the_log_is_an_error() {
    let dir = temp_dir("log-unwritable-input");
    let blocker = temp_dir("log-unwritable-output").join("not-a-dir");
    std::fs::write(&blocker, "x").unwrap();

    assert!(matches!(
        process_dir_with_log(&dir, true, &blocker),
        Err(EdiError::Write { .. })
    ));
    assert!(matches!(
        parse_dir(&dir, &blocker, false),
        Err(EdiError::Write { .. })
    ));
}

#[test]
fn summary_without_out_reports_counts_and_writes_no_log() {
    let dir = temp_dir("log-cli-input");
    let cwd = temp_dir("log-cli-cwd");
    std::fs::write(dir.join("bad.edi"), two_bad_claims()).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_edi-parse"))
        .arg("summary")
        .arg(&dir)
        .current_dir(&cwd)
        .output()
        .unwrap();
    assert!(output.status.success());

    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        summary["validation_error_counts"],
        json!({ "charge_mismatch": 1, "no_service_lines": 1 })
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("use --out"));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    assert_eq!(std::fs::read_dir(&cwd).unwrap().count(), 0);
}
