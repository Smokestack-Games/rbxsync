//! Extraction benchmarks using Criterion

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rbxsync_core::{Instance, PropertyValue};

fn build_game_tree(services: &[&str], instances_per_service: usize) -> Instance {
    let mut root = Instance::new("DataModel", "game");
    for service in services {
        let mut svc = Instance::new(service, service);
        for i in 0..instances_per_service {
            let mut folder = Instance::new("Folder", &format!("Folder_{}", i));
            for j in 0..5 {
                let mut script = Instance::new("Script", &format!("Script_{}_{}", i, j));
                script.set_property("Source", PropertyValue::String("return {}".to_string()));
                folder.add_child(script);
            }
            svc.add_child(folder);
        }
        root.add_child(svc);
    }
    root
}

fn count_instances(root: &Instance) -> usize {
    1 + root.children.iter().map(count_instances).sum::<usize>()
}

fn extraction_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("extraction");

    group.bench_function("build_small_game_tree", |b| {
        b.iter(|| black_box(build_game_tree(&["ServerScriptService", "ReplicatedStorage"], 10)))
    });

    group.bench_function("build_large_game_tree", |b| {
        b.iter(|| black_box(build_game_tree(&["ServerScriptService", "ReplicatedStorage", "ServerStorage", "StarterGui", "Workspace"], 50)))
    });

    let tree = build_game_tree(&["ServerScriptService", "ReplicatedStorage"], 20);
    group.throughput(Throughput::Elements(count_instances(&tree) as u64));
    group.bench_function("serialize_game_tree", |b| {
        b.iter(|| black_box(serde_json::to_string(&tree).unwrap()))
    });

    group.finish();
}

criterion_group!(benches, extraction_benchmarks);
criterion_main!(benches);
