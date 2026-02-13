# RbxSync Build Automation
# Usage:
#   make           - Build all Rust crates
#   make install   - Build and install CLI binary
#   make all       - Build everything (Rust + VS Code + Plugin)
#   make check     - Run clippy, tests, and format check
#   make clean     - Remove build artifacts

.PHONY: all build install build-vscode build-plugin check test clippy fmt clean help

# Default: build Rust crates
build:
	cargo build --release

# Build and install CLI to PATH
install: build
	cargo install --path rbxsync-cli --force

# Build everything (Rust + VS Code extension + Plugin)
all: build build-vscode build-plugin

# Build VS Code extension
build-vscode:
	cd rbxsync-vscode && npm ci && npm run build && npx vsce package

# Build Roblox Studio plugin (.rbxm)
build-plugin: build
	./target/release/rbxsync build-plugin

# Run all quality checks
check: clippy test fmt

# Run tests
test:
	cargo test

# Run clippy lints
clippy:
	cargo clippy -- -D warnings

# Check formatting
fmt:
	cargo fmt -- --check

# Format code
fmt-fix:
	cargo fmt

# Remove build artifacts
clean:
	cargo clean
	rm -rf build/RbxSync.rbxm
	rm -rf rbxsync-vscode/*.vsix

# Show help
help:
	@echo "RbxSync Build Targets:"
	@echo "  make            Build all Rust crates (release)"
	@echo "  make install    Build and install CLI binary"
	@echo "  make all        Build everything (Rust + VS Code + Plugin)"
	@echo "  make check      Run clippy, tests, and format check"
	@echo "  make test       Run cargo tests"
	@echo "  make clippy     Run clippy lints"
	@echo "  make fmt        Check formatting"
	@echo "  make fmt-fix    Auto-format code"
	@echo "  make clean      Remove build artifacts"
	@echo ""
	@echo "Component targets:"
	@echo "  make build-vscode   Build VS Code extension (.vsix)"
	@echo "  make build-plugin   Build Roblox Studio plugin (.rbxm)"
