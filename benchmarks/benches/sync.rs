//! Sync benchmarks using Criterion

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rbxsync_core::{Instance, PropertyValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncOperation {
    operation: String,
    path: String,
    instance: Option<Instance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncBatch {
    operations: Vec<SyncOperation>,
    project_dir: String,
}

fn create_sync_operations(count: usize) -> Vec<SyncOperation> {
    (0..count).map(|i| {
        let mut inst = Instance::new("ModuleScript", &format!("Script_{}", i));
        inst.set_property("Source", PropertyValue::String("return {}".to_string()));
        SyncOperation { operation: "update".to_string(), path: format!("ServerScriptService.Script_{}", i), instance: Some(inst) }
    }).collect()
}

fn sync_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync");

    let small = SyncBatch { operations: create_sync_operations(10), project_dir: "/test".to_string() };
    group.bench_function("serialize_small_sync_batch", |b| b.iter(|| black_box(serde_json::to_string(&small).unwrap())));

    let large = SyncBatch { operations: create_sync_operations(200), project_dir: "/test".to_string() };
    let size = serde_json::to_string(&large).unwrap().len();
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function("serialize_large_sync_batch", |b| b.iter(|| black_box(serde_json::to_string(&large).unwrap())));

    group.finish();
}

criterion_group!(benches, sync_benchmarks);
criterion_main!(benches);
