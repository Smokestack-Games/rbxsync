//! Instance tree benchmarks

use crate::{run_benchmark, BenchmarkResult};
use rbxsync_core::Instance;

const CATEGORY: &str = "Instance Tree";
const ITERATIONS: u32 = 50;

pub fn run_all() -> Vec<BenchmarkResult> {
    let mut results = Vec::new();
    results.push(bench_build_flat_tree());
    results.push(bench_build_nested_tree());
    results.push(bench_traverse_tree());
    results.push(bench_clone_tree());
    results
}

fn build_flat_tree(size: usize) -> Instance {
    let mut root = Instance::new("Workspace", "Workspace");
    for i in 0..size {
        root.add_child(Instance::new("Part", &format!("Part_{}", i)));
    }
    root
}

fn build_nested_tree(breadth: usize, depth: usize) -> Instance {
    fn build_level(name: &str, breadth: usize, remaining_depth: usize) -> Instance {
        let mut instance = Instance::new("Folder", name);
        if remaining_depth > 0 {
            for i in 0..breadth {
                instance.add_child(build_level(&format!("{}_child_{}", name, i), breadth, remaining_depth - 1));
            }
        }
        instance
    }
    build_level("Root", breadth, depth)
}

fn traverse_dfs(root: &Instance) -> usize {
    let mut count = 1;
    for child in &root.children {
        count += traverse_dfs(child);
    }
    count
}

fn clone_tree(root: &Instance) -> Instance {
    let mut cloned = Instance::new(&root.class_name, &root.name);
    cloned.properties = root.properties.clone();
    for child in &root.children {
        cloned.add_child(clone_tree(child));
    }
    cloned
}

fn bench_build_flat_tree() -> BenchmarkResult {
    run_benchmark("Build flat tree (1000 children)", CATEGORY, ITERATIONS, || {
        let tree = build_flat_tree(1000);
        std::hint::black_box(tree);
    })
}

fn bench_build_nested_tree() -> BenchmarkResult {
    run_benchmark("Build nested tree (5x4 = 625 nodes)", CATEGORY, ITERATIONS, || {
        let tree = build_nested_tree(5, 4);
        std::hint::black_box(tree);
    })
}

fn bench_traverse_tree() -> BenchmarkResult {
    let tree = build_nested_tree(5, 4);
    run_benchmark("Traverse tree DFS (~625 nodes)", CATEGORY, ITERATIONS, || {
        let count = traverse_dfs(&tree);
        std::hint::black_box(count);
    })
}

fn bench_clone_tree() -> BenchmarkResult {
    let tree = build_nested_tree(5, 4);
    run_benchmark("Clone tree (~625 nodes)", CATEGORY, ITERATIONS, || {
        let cloned = clone_tree(&tree);
        std::hint::black_box(cloned);
    })
}
