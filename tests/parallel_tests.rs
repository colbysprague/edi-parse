//! Task 8: `collect_files`, `process_file`, and `process_dir` with rayon.

mod common;

use std::path::Path;

use common::*;
use edi_parse::{collect_files, parse_dir, process_dir, process_file, EdiError, Summary};

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
fn parse_dir_on_missing_dir() {
    let out = temp_dir("parse-dir-missing");
    assert!(matches!(
        parse_dir(Path::new("/definitely/not/a/real/dir"), &out, false),
        Err(EdiError::Io { .. })
    ));
}
