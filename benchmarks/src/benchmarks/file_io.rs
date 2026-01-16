//! File I/O benchmarks

use crate::{run_benchmark, run_benchmark_with_throughput, BenchmarkResult};
use std::fs;
use std::io::{Read, Write};
use tempfile::TempDir;

const CATEGORY: &str = "File I/O";
const ITERATIONS: u32 = 50;

pub fn run_all() -> Vec<BenchmarkResult> {
    let mut results = Vec::new();
    results.push(bench_write_small_files());
    results.push(bench_read_small_files());
    results.push(bench_write_medium_files());
    results.push(bench_read_medium_files());
    results.push(bench_write_file_batch());
    results.push(bench_create_directory_tree());
    results
}

fn bench_write_small_files() -> BenchmarkResult {
    let temp_dir = TempDir::new().unwrap();
    let content = generate_lua_content(1024);
    let bytes = content.len() as u64;

    run_benchmark_with_throughput("Write small file (1KB)", CATEGORY, ITERATIONS, bytes, || {
        let path = temp_dir.path().join("test.luau");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.sync_all().unwrap();
    })
}

fn bench_read_small_files() -> BenchmarkResult {
    let temp_dir = TempDir::new().unwrap();
    let content = generate_lua_content(1024);
    let path = temp_dir.path().join("test.luau");
    fs::write(&path, &content).unwrap();
    let bytes = content.len() as u64;

    run_benchmark_with_throughput("Read small file (1KB)", CATEGORY, ITERATIONS, bytes, || {
        let mut file = fs::File::open(&path).unwrap();
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap();
        std::hint::black_box(buffer);
    })
}

fn bench_write_medium_files() -> BenchmarkResult {
    let temp_dir = TempDir::new().unwrap();
    let content = generate_lua_content(10 * 1024);
    let bytes = content.len() as u64;

    run_benchmark_with_throughput("Write medium file (10KB)", CATEGORY, ITERATIONS, bytes, || {
        let path = temp_dir.path().join("test.luau");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.sync_all().unwrap();
    })
}

fn bench_read_medium_files() -> BenchmarkResult {
    let temp_dir = TempDir::new().unwrap();
    let content = generate_lua_content(10 * 1024);
    let path = temp_dir.path().join("test.luau");
    fs::write(&path, &content).unwrap();
    let bytes = content.len() as u64;

    run_benchmark_with_throughput("Read medium file (10KB)", CATEGORY, ITERATIONS, bytes, || {
        let mut file = fs::File::open(&path).unwrap();
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap();
        std::hint::black_box(buffer);
    })
}

fn bench_write_file_batch() -> BenchmarkResult {
    let temp_dir = TempDir::new().unwrap();
    let files: Vec<(String, String)> = (0..100)
        .map(|i| (format!("script_{}.luau", i), generate_lua_content(1024)))
        .collect();
    let total_bytes: u64 = files.iter().map(|(_, c)| c.len() as u64).sum();

    run_benchmark_with_throughput("Write file batch (100 x 1KB)", CATEGORY, ITERATIONS / 5, total_bytes, || {
        for (name, content) in &files {
            let path = temp_dir.path().join(name);
            fs::write(&path, content).unwrap();
        }
    })
}

fn bench_create_directory_tree() -> BenchmarkResult {
    let temp_dir = TempDir::new().unwrap();

    run_benchmark("Create directory tree (5 levels)", CATEGORY, ITERATIONS, || {
        let base = temp_dir.path().join(format!("tree_{}", rand::random::<u32>()));
        let deep_path = base.join("ServerScriptService").join("Modules").join("Core").join("Utils");
        fs::create_dir_all(&deep_path).unwrap();
    })
}

fn generate_lua_content(size: usize) -> String {
    let template = "-- Auto-generated benchmark file\nlocal Module = {}\nreturn Module\n";
    let mut content = String::with_capacity(size);
    while content.len() < size {
        content.push_str(template);
    }
    content.truncate(size);
    content
}
