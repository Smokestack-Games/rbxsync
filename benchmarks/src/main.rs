//! RbxSync Benchmark Runner

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{Duration, Instant};

mod benchmarks;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub category: String,
    pub iterations: u32,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub std_dev_ms: f64,
    pub throughput: Option<Throughput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Throughput {
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub timestamp: String,
    pub version: String,
    pub system_info: SystemInfo,
    pub results: Vec<BenchmarkResult>,
    pub summary: BenchmarkSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub total_benchmarks: usize,
    pub categories: Vec<CategorySummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CategorySummary {
    pub name: String,
    pub benchmark_count: usize,
    pub total_time_ms: f64,
}

pub fn run_benchmark<F>(name: &str, category: &str, iterations: u32, mut f: F) -> BenchmarkResult
where
    F: FnMut(),
{
    let mut times: Vec<Duration> = Vec::with_capacity(iterations as usize);
    f(); // Warmup
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        times.push(start.elapsed());
    }
    let times_ms: Vec<f64> = times.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    let mean_ms = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
    let min_ms = times_ms.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_ms = times_ms.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let variance = times_ms.iter().map(|t| (t - mean_ms).powi(2)).sum::<f64>() / times_ms.len() as f64;
    let std_dev_ms = variance.sqrt();

    BenchmarkResult { name: name.to_string(), category: category.to_string(), iterations, mean_ms, min_ms, max_ms, std_dev_ms, throughput: None }
}

pub fn run_benchmark_with_throughput<F>(name: &str, category: &str, iterations: u32, bytes: u64, mut f: F) -> BenchmarkResult
where
    F: FnMut(),
{
    let mut result = run_benchmark(name, category, iterations, || f());
    let bytes_per_sec = (bytes as f64 * 1000.0) / result.mean_ms;
    result.throughput = Some(Throughput { value: bytes_per_sec / (1024.0 * 1024.0), unit: "MB/s".to_string() });
    result
}

fn print_report(report: &BenchmarkReport) {
    println!("\n======== RbxSync Benchmark Report ========");
    println!("Version: {} | {} ({})", report.version, report.system_info.os, report.system_info.arch);
    println!();
    let mut current_cat = String::new();
    for r in &report.results {
        if r.category != current_cat {
            current_cat = r.category.clone();
            println!("--- {} ---", current_cat);
        }
        print!("  {:<40} {:>8.3}ms (±{:.3})", r.name, r.mean_ms, r.std_dev_ms);
        if let Some(ref tp) = r.throughput {
            print!(" [{:.2} {}]", tp.value, tp.unit);
        }
        println!();
    }
    println!("\nTotal: {} benchmarks", report.summary.total_benchmarks);
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let json_only = args.iter().any(|a| a == "--json-only");

    println!("Running RbxSync benchmarks...\n");
    let mut results = Vec::new();

    println!("File I/O benchmarks...");
    results.extend(benchmarks::file_io::run_all());

    println!("Serialization benchmarks...");
    results.extend(benchmarks::serialization::run_all());

    println!("Instance tree benchmarks...");
    results.extend(benchmarks::instance_tree::run_all());

    let mut categories: std::collections::HashMap<String, (usize, f64)> = std::collections::HashMap::new();
    for r in &results {
        let e = categories.entry(r.category.clone()).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += r.mean_ms;
    }

    let report = BenchmarkReport {
        timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        system_info: SystemInfo { os: std::env::consts::OS.to_string(), arch: std::env::consts::ARCH.to_string() },
        results,
        summary: BenchmarkSummary {
            total_benchmarks: categories.values().map(|c| c.0).sum(),
            categories: categories.into_iter().map(|(n, (c, t))| CategorySummary { name: n, benchmark_count: c, total_time_ms: t }).collect(),
        },
    };

    if json_only {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
        fs::create_dir_all("benchmarks/results")?;
        let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        fs::write(format!("benchmarks/results/benchmark-{}.json", ts), serde_json::to_string_pretty(&report)?)?;
        println!("\nResults saved to benchmarks/results/");
    }
    Ok(())
}
