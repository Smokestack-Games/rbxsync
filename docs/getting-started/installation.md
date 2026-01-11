# Installation

## Prerequisites

::: details macOS / Linux (click to expand)
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Restart your terminal, then verify:
rustc --version
cargo --version

# For VS Code extension: Install Node.js from https://nodejs.org
```
:::

::: details Windows (click to expand)
**Step 1:** Install Visual Studio Build Tools (REQUIRED for Rust)
- Download from [visualstudio.microsoft.com/visual-cpp-build-tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- Run the installer
- Select **"Desktop development with C++"** workload
- Complete the installation

**Step 2:** Install Rust
- Download from [rustup.rs](https://rustup.rs)
- Run `rustup-init.exe`
- Follow the prompts (default options are fine)

**Step 3:** Restart your terminal and verify:
```powershell
rustc --version
cargo --version
```

**Step 4:** For VS Code extension, install Node.js from https://nodejs.org
:::

## 1. Build CLI from Source

::: code-group

```bash [macOS / Linux]
# Clone and build
git clone https://github.com/devmarissa/rbxsync
cd rbxsync
cargo build --release

# Add to PATH (optional)
cp target/release/rbxsync /usr/local/bin/
```

```powershell [Windows]
# Clone and build (PowerShell or Git Bash)
git clone https://github.com/devmarissa/rbxsync
cd rbxsync
cargo build --release

# Add to PATH (PowerShell as Admin):
Copy-Item target\release\rbxsync.exe C:\Windows\System32\

# Or add the target\release folder to your PATH environment variable
```

:::

## 2. Install Studio Plugin

**Option A: Roblox Creator Store (recommended)**

Install from the [Roblox Creator Store](https://create.roblox.com/store/asset/105132526235830/RbxSync) - updates automatically.

**Option B: Download from GitHub**
1. Download `RbxSync.rbxm` from [GitHub Releases](https://github.com/devmarissa/rbxsync/releases)
2. Copy to your plugins folder:
   - **macOS:** `~/Documents/Roblox/Plugins/`
   - **Windows:** `%LOCALAPPDATA%\Roblox\Plugins\`

**Option C: Build from source**

::: warning
This requires completing Step 1 above AND adding rbxsync to your PATH.
:::

```bash
rbxsync build-plugin --install
```

## 3. Install VS Code Extension (Optional)

**Option A: VS Code Marketplace (recommended)**

Install from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=rbxsync.rbxsync) or search "RbxSync" in the VS Code Extensions panel.

**Option B: Download from GitHub**
1. Download `rbxsync-*.vsix` from [GitHub Releases](https://github.com/devmarissa/rbxsync/releases)
2. In VS Code: `Cmd+Shift+P` (Mac) or `Ctrl+Shift+P` (Windows)
3. Type "Extensions: Install from VSIX"
4. Select the downloaded `.vsix` file

**Option C: Build from source**

::: warning
This requires Node.js installed.
:::

```bash
cd rbxsync-vscode
npm install
npm run package
code --install-extension rbxsync-*.vsix
```

## Verify Installation

```bash
rbxsync version
```

You should see the version number and git commit hash.

## Next Steps

- [Quick Start](/getting-started/quick-start) - Create your first project
