//! Serialization benchmarks

use crate::{run_benchmark_with_throughput, BenchmarkResult};
use rbxsync_core::{Instance, PropertyValue, Vector3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const CATEGORY: &str = "Serialization";
const ITERATIONS: u32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncPayload {
    instances: Vec<Instance>,
    metadata: HashMap<String, String>,
}

pub fn run_all() -> Vec<BenchmarkResult> {
    let mut results = Vec::new();
    results.push(bench_serialize_single_instance());
    results.push(bench_deserialize_single_instance());
    results.push(bench_serialize_small_batch());
    results.push(bench_serialize_large_batch());
    results
}

fn create_test_instance(name: &str) -> Instance {
    let mut instance = Instance::new("Part", name);
    instance.set_property("Position", PropertyValue::Vector3(Vector3 { x: 10.5, y: 20.0, z: -5.25 }));
    instance.set_property("Name", PropertyValue::String(name.to_string()));
    instance
}

fn bench_serialize_single_instance() -> BenchmarkResult {
    let instance = create_test_instance("TestPart");
    let json = serde_json::to_string(&instance).unwrap();
    let bytes = json.len() as u64;

    run_benchmark_with_throughput("Serialize single instance", CATEGORY, ITERATIONS, bytes, || {
        let json = serde_json::to_string(&instance).unwrap();
        std::hint::black_box(json);
    })
}

fn bench_deserialize_single_instance() -> BenchmarkResult {
    let instance = create_test_instance("TestPart");
    let json = serde_json::to_string(&instance).unwrap();
    let bytes = json.len() as u64;

    run_benchmark_with_throughput("Deserialize single instance", CATEGORY, ITERATIONS, bytes, || {
        let parsed: Instance = serde_json::from_str(&json).unwrap();
        std::hint::black_box(parsed);
    })
}

fn bench_serialize_small_batch() -> BenchmarkResult {
    let batch: Vec<Instance> = (0..10).map(|i| create_test_instance(&format!("Instance_{}", i))).collect();
    let payload = SyncPayload { instances: batch, metadata: HashMap::from([("version".to_string(), "1.0".to_string())]) };
    let json = serde_json::to_string(&payload).unwrap();
    let bytes = json.len() as u64;

    run_benchmark_with_throughput("Serialize small batch (10)", CATEGORY, ITERATIONS, bytes, || {
        let json = serde_json::to_string(&payload).unwrap();
        std::hint::black_box(json);
    })
}

fn bench_serialize_large_batch() -> BenchmarkResult {
    let batch: Vec<Instance> = (0..1000).map(|i| create_test_instance(&format!("Instance_{}", i))).collect();
    let payload = SyncPayload { instances: batch, metadata: HashMap::from([("version".to_string(), "1.0".to_string())]) };
    let json = serde_json::to_string(&payload).unwrap();
    let bytes = json.len() as u64;

    run_benchmark_with_throughput("Serialize large batch (1000)", CATEGORY, ITERATIONS / 10, bytes, || {
        let json = serde_json::to_string(&payload).unwrap();
        std::hint::black_box(json);
    })
}
