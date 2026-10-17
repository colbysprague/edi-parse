//! Shared helpers for the integration tests. Each test file uses a subset.
#![allow(dead_code)]

use std::path::PathBuf;

pub const S_837_MULTI: &str = "837p_multi_claim.edi";
pub const S_837_PIPE: &str = "837p_pipe_delims.edi";
pub const S_835_MULTI: &str = "835_multi_clp.edi";
pub const S_835_CRLF: &str = "835_crlf_denial.edi";
pub const ALL_SAMPLES: [&str; 4] = [S_837_MULTI, S_837_PIPE, S_835_MULTI, S_835_CRLF];

pub fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
}

pub fn samples_dir() -> PathBuf {
    data_dir().join("samples")
}

pub fn invalid_dir() -> PathBuf {
    data_dir().join("invalid")
}

pub fn sample(name: &str) -> String {
    let path = samples_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

pub fn invalid(name: &str) -> String {
    let path = invalid_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// A complete 106-byte ISA segment (terminator included) with the given delimiters.
pub fn isa(element: char, repetition: char, component: char, segment: char) -> String {
    let e = element;
    let s = format!(
        "ISA{e}00{e}          {e}00{e}          {e}ZZ{e}SENDERID       {e}ZZ{e}RECEIVERID     \
         {e}261017{e}1200{e}{repetition}{e}00501{e}000000001{e}0{e}T{e}{component}{segment}"
    );
    assert_eq!(s.len(), 106, "test helper built a bad ISA");
    s
}

/// Wrap `body` segments (standard delimiters, each without its `~`) in
/// ISA/GS ... GE/IEA. `body` should include its own ST/SE.
pub fn envelope(functional_id: &str, body: &[&str]) -> String {
    let mut out = isa('*', '^', ':', '~');
    out.push_str(&format!(
        "GS*{functional_id}*SENDERID*RECEIVERID*20261017*1200*1*X*005010~"
    ));
    for seg in body {
        out.push_str(seg);
        out.push('~');
    }
    out.push_str("GE*1*1~IEA*1*000000001~");
    out
}

pub fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[track_caller]
pub fn assert_approx(actual: f64, expected: f64) {
    assert!(
        approx(actual, expected),
        "expected {expected}, got {actual}"
    );
}

/// A fresh, empty temp directory unique to this test.
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("edi-parse-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
