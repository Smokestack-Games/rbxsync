# Contributing to RbxSync

Thanks for your interest in contributing!

## Development Setup

```bash
# Clone the repo
git clone https://github.com/Smokestack-Games/rbxsync
cd rbxsync

# Build all components
cargo build --release

# Build VS Code extension
cd rbxsync-vscode && npm install && npm run build

# Build and install Studio plugin
rbxsync build-plugin --install
```

## Project Structure

- `rbxsync-core/` - Shared Rust types and serialization
- `rbxsync-server/` - HTTP server for Studio communication
- `rbxsync-cli/` - Command-line interface
- `rbxsync-mcp/` - MCP server for AI integration
- `rbxsync-vscode/` - VS Code extension
- `plugin/` - Roblox Studio plugin (Luau)

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Commit your changes
6. Push to your fork
7. Open a Pull Request

## Code Style

- Rust: Follow standard `rustfmt` formatting
- TypeScript: Use the project's ESLint config
- Luau: Use consistent 4-space indentation

## Questions?

Open an issue or start a discussion on GitHub.
