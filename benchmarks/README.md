# RbxSync Benchmarks

Performance benchmarks for RbxSync v1.2 release.

## Running Benchmarks

### Quick Benchmarks

```bash
# Run all benchmarks with human-readable + JSON output
cargo run -p rbxsync-benchmarks --release

# Quick mode (fewer iterations)
cargo run -p rbxsync-benchmarks --release -- --quick

# Full mode (more iterations)
cargo run -p rbxsync-benchmarks --release -- --full

# JSON only output
cargo run -p rbxsync-benchmarks --release -- --json-only
```

### Criterion Benchmarks

```bash
# Run all criterion benchmarks
cargo bench -p rbxsync-benchmarks

# Run specific benchmark group
cargo bench -p rbxsync-benchmarks -- extraction
cargo bench -p rbxsync-benchmarks -- sync
cargo bench -p rbxsync-benchmarks -- file_io
```

## Benchmark Categories

### File I/O
- Small file writes (100 files ~100 bytes)
- Medium file writes (50 files ~10KB)
- File reads
- Directory traversal

### JSON Serialization
- Serialize/deserialize small payloads (3 instances)
- Serialize/deserialize large payloads (1000 instances)

### Sync Operations
- Build file tree from directory
- Path normalization
- Change detection

## Output

Results saved to `benchmarks/results/`:
- `benchmark-TIMESTAMP.json` - Machine-readable
- `benchmark-TIMESTAMP.txt` - Human-readable

## Performance Targets (v1.2)

| Operation | Target |
|-----------|--------|
| Small file writes (100 files) | < 50ms |
| Medium file writes (50 files) | < 100ms |
| JSON serialize small | < 1ms |
| JSON serialize large | < 50ms |
