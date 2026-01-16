//! File I/O benchmarks using Criterion

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::fs;
use tempfile::TempDir;

fn generate_content(size: usize) -> String {
    "-- Lua content\nlocal x = 1\nreturn x\n".repeat(size / 30 + 1)[..size].to_string()
}

fn file_io_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_io");

    let small = generate_content(1024);
    group.throughput(Throughput::Bytes(small.len() as u64));
    group.bench_function("write_small_file_1kb", |b| {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.luau");
        b.iter(|| { fs::write(&path, &small).unwrap(); black_box(()) })
    });

    let large = generate_content(100 * 1024);
    group.throughput(Throughput::Bytes(large.len() as u64));
    group.bench_function("write_large_file_100kb", |b| {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.luau");
        b.iter(|| { fs::write(&path, &large).unwrap(); black_box(()) })
    });

    group.bench_function("write_batch_100_files", |b| {
        let dir = TempDir::new().unwrap();
        let files: Vec<_> = (0..100).map(|i| (format!("s_{}.luau", i), generate_content(1024))).collect();
        b.iter(|| {
            for (n, c) in &files { fs::write(dir.path().join(n), c).unwrap(); }
            black_box(())
        })
    });

    group.finish();
}

criterion_group!(benches, file_io_benchmarks);
criterion_main!(benches);
