//! Task 9: benchmarks. Run with `make gen-data N=1000 && make bench`.
//!
//! - `parse_str/*`: single-file parse speed on each sample.
//! - `process_dir/*`: sequential vs parallel on `data/generated`.

use std::hint::black_box;
use std::path::{Path, PathBuf};

use criterion::{criterion_group, criterion_main, Criterion};
use edi_parse::{parse_str, process_dir};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn bench_parse_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_str");
    for name in [
        "837p_multi_claim.edi",
        "837p_pipe_delims.edi",
        "835_multi_clp.edi",
        "835_crlf_denial.edi",
    ] {
        let input = std::fs::read_to_string(data_dir().join("samples").join(name))
            .expect("sample file should exist");
        group.bench_function(name, |b| {
            b.iter(|| parse_str(black_box(&input)).expect("sample should parse"))
        });
    }
    group.finish();
}

fn bench_process_dir(c: &mut Criterion) {
    let dir = data_dir().join("generated");
    if !dir.is_dir() {
        eprintln!(
            "skipping process_dir benchmarks: {} not found (run `make gen-data`)",
            dir.display()
        );
        return;
    }

    let mut group = c.benchmark_group("process_dir");
    group.sample_size(10);
    group.bench_function("sequential", |b| {
        b.iter(|| process_dir(black_box(&dir), false).expect("process_dir"))
    });
    group.bench_function("parallel", |b| {
        b.iter(|| process_dir(black_box(&dir), true).expect("process_dir"))
    });
    group.finish();
}

criterion_group!(benches, bench_parse_str, bench_process_dir);
criterion_main!(benches);
