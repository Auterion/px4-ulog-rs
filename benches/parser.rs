//! Criterion benchmarks for the two parser entry points.
//!
//! `cargo bench` here answers "did a change regress parsing throughput?" with
//! statistical rigor. The apples-to-apples comparison against ulog_cpp and
//! pyulog lives in `examples/bench.rs` / `scripts/benchmark_compare.sh`, which
//! measure raw wall-clock the same way those tools do.
//!
//! Fixtures that are not present are skipped rather than failing the run, so a
//! checkout without the larger sample logs can still benchmark what it has.

use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use px4_ulog::stream_parser::file_reader::{read_file_with_simple_callback, SimpleCallbackResult};

const STREAM_FIXTURES: &[&str] = &[
    "sample.ulg",
    "quadrotor_local.ulg",
    "fixed_wing_gps.ulg",
    "vtol_demo.ulg",
    "truncated_real.ulg",
    "sample_appended.ulg",
];

const FULL_FIXTURES: &[&str] = &["fixed_wing_gps.ulg", "sample.ulg"];

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

fn count_via_stream(path: &str) -> usize {
    let mut count = 0usize;
    read_file_with_simple_callback(path, &mut |_| {
        count += 1;
        SimpleCallbackResult::KeepReading
    })
    .expect("fixture should parse without error");
    count
}

fn bench_stream_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_parser");
    for name in STREAM_FIXTURES {
        let path = fixture_path(name);
        if !Path::new(&path).exists() {
            continue;
        }
        let size = std::fs::metadata(&path).unwrap().len();
        group.throughput(Throughput::Bytes(size));
        group.bench_function(*name, |b| {
            b.iter(|| black_box(count_via_stream(black_box(&path))));
        });
    }
    group.finish();
}

fn bench_full_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_parser");
    for name in FULL_FIXTURES {
        let path = fixture_path(name);
        if !Path::new(&path).exists() {
            continue;
        }
        let size = std::fs::metadata(&path).unwrap().len();
        group.throughput(Throughput::Bytes(size));
        group.bench_function(*name, |b| {
            b.iter(|| black_box(px4_ulog::full_parser::read_file(black_box(&path)).unwrap()));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_stream_parser, bench_full_parser);
criterion_main!(benches);
