# Installation

RbxSync requires three components:
1. **CLI** - Runs the sync server (required)
2. **Studio Plugin** - Connects Roblox Studio to the server
3. **VS Code Extension** - Optional, provides editor integration

---

## 1. Install CLI (Required)

The CLI runs the sync server that bridges Studio and your filesystem.

### Quick Install (Recommended)

**macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.ps1 | iex
```

Then restart your terminal and verify:
```bash
rbxsync version
```

### Alternative: Manual Download

Download pre-built binaries from [GitHub Releases](https://github.com/devmarissa/rbxsync/releases):

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | `rbxsync-macos-aarch64` |
| macOS (Intel) | `rbxsync-macos-x86_64` |
| Windows | `rbxsync-windows-x86_64.exe` |

**macOS:** Move to `/usr/local/bin/` and run `chmod +x rbxsync`

**Windows:** Move to a folder in your PATH (e.g., `%LOCALAPPDATA%\rbxsync\`), or add the download location to PATH

::: details Build from Source (Advanced)

**macOS:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Restart terminal, then clone and build
git clone https://github.com/devmarissa/rbxsync
cd rbxsync
cargo build --release

# Add to PATH
sudo cp target/release/rbxsync /usr/local/bin/
```

**Windows:**
1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++"
2. Install Rust from [rustup.rs](https://rustup.rs)
3. Restart terminal, then:
```powershell
git clone https://github.com/devmarissa/rbxsync
cd rbxsync
cargo build --release
```
4. Add `target\release` to your PATH
:::

---

## 2. Install Studio Plugin

Choose one option:

**Option A: Download from GitHub (Recommended)**

1. Download `RbxSync.rbxm` from [GitHub Releases](https://github.com/devmarissa/rbxsync/releases)
2. Copy to your plugins folder:
   - **macOS:** `~/Documents/Roblox/Plugins/`
   - **Windows:** `%LOCALAPPDATA%\Roblox\Plugins\`

::: tip Finding the Windows plugins folder
Press `Win + R`, paste `%LOCALAPPDATA%\Roblox\Plugins\`, and press Enter.
:::

**Option B: CLI Install**

If you have the CLI installed, you can download and install the plugin directly:
```bash
rbxsync plugin install
```

**Option C: Build from source**

```bash
rbxsync build-plugin --install
```

---

## 3. Install VS Code Extension (Optional)

**Option A: VS Code Marketplace (Recommended)**

Install from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=rbxsync.rbxsync) or search "RbxSync" in the Extensions panel.

The extension will automatically run `rbxsync serve` when you connect.

**Option B: Open VSX (for VSCodium, Cursor, Antigravity, etc.)**

If you're using a VS Code fork like VSCodium, Cursor, Antigravity, or Windsurf, install from [Open VSX](https://open-vsx.org/extension/rbxsync/rbxsync).

**Option C: Download from GitHub**

1. Download `rbxsync-*.vsix` from [GitHub Releases](https://github.com/devmarissa/rbxsync/releases)
2. In VS Code: `Ctrl+Shift+P` (Windows) or `Cmd+Shift+P` (Mac)
3. Type "Extensions: Install from VSIX"
4. Select the downloaded `.vsix` file

---

## Updating

::: warning Updates are NOT automatic
Both the Studio plugin and VS Code extension require manual updates. They will NOT auto-update.
:::

### Update CLI

```bash
rbxsync update
```

### Update Studio Plugin

**Using CLI (Recommended):**
```bash
rbxsync plugin install
```
This downloads the latest plugin from GitHub releases and installs it. Then restart Studio.

**Manually:**
1. Download the latest `RbxSync.rbxm` from [GitHub Releases](https://github.com/devmarissa/rbxsync/releases)
2. Copy to your plugins folder (same as installation)
3. Restart Studio

### Update VS Code Extension

**If installed from Marketplace:**
1. Open VS Code
2. Go to **Extensions** (Ctrl/Cmd+Shift+X)
3. Find RbxSync and click **Update** if available
4. Restart VS Code

**If installed manually:**
```bash
rbxsync update --vscode
code --install-extension rbxsync-vscode/rbxsync-*.vsix
```
Then restart VS Code.

---

## Troubleshooting

### Windows: 'rbxsync' is not recognized

The CLI isn't in your PATH. Either:
- Follow the "Add to PATH" steps above carefully
- Or use the full path: `.\target\release\rbxsync.exe`

### Windows: Build fails with linker errors

Make sure you installed Visual Studio Build Tools with the **"Desktop development with C++"** workload. Restart your terminal after installation.

### Windows: cargo not found

Restart your terminal after installing Rust. If it still doesn't work, run the Rust installer again.

### Plugin not appearing in Studio

1. Restart Roblox Studio completely (not just the place)
2. Check the Plugins tab in the ribbon
3. If using manual install, verify the `.rbxm` file is in the correct plugins folder

### Server won't start

- Check if port 44755 is already in use: `lsof -i :44755` (Mac) or `netstat -an | findstr 44755` (Windows)
- Try stopping existing server: `rbxsync stop`

---

## Next Steps

- [Quick Start](/getting-started/quick-start) - Create your first project
