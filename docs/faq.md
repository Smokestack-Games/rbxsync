# Frequently Asked Questions

## Installation

### Windows: "linker `link.exe` not found" error

This error means you need to install the Visual Studio Build Tools before building RbxSync.

**Solution:**
1. Download [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
2. Run the installer
3. Select **"Desktop development with C++"**
4. Complete installation and restart your terminal
5. Try `cargo build --release` again

::: tip Alternative: Use GNU toolchain
If you prefer not to install Visual Studio Build Tools, you can use the GNU toolchain:
```powershell
rustup default stable-x86_64-pc-windows-gnu
```
This requires [MSYS2](https://www.msys2.org/) with `pacman -S mingw-w64-x86_64-toolchain`
:::

### macOS: "command not found: rbxsync"

The binary isn't in your PATH. Either:
- Copy it: `cp target/release/rbxsync /usr/local/bin/`
- Or use the full path: `./target/release/rbxsync`

### Windows: "rbxsync is not recognized"

Add the binary to your PATH:
```powershell
# PowerShell as Admin:
Copy-Item target\release\rbxsync.exe C:\Windows\System32\
```
Or add `C:\path\to\rbxsync\target\release` to your PATH environment variable.

### Windows: How do I remove an old RbxSync from PATH?

If you installed a new version but the old one is still being used:

1. **Find where the old version is:**
   ```powershell
   where.exe rbxsync
   ```
   This shows all locations where `rbxsync.exe` exists.

2. **Remove the old installation:**
   - Delete the old `rbxsync.exe` file from the path shown above
   - Or remove that folder from your PATH environment variable

3. **Edit PATH (if needed):**
   - Press `Win + R`, type `sysdm.cpl`, press Enter
   - Go to **Advanced** → **Environment Variables**
   - Under "User variables", find **Path** and click **Edit**
   - Remove any old RbxSync directories
   - Click **OK** to save

4. **Restart your terminal** and verify:
   ```powershell
   rbxsync version
   ```

::: warning System32 requires Admin rights
If the old version is in `C:\Windows\System32\`, you need Administrator privileges to delete it:
1. Open PowerShell **as Administrator** (right-click → Run as Administrator)
2. Run: `Remove-Item C:\Windows\System32\rbxsync.exe`
3. Or manually delete via File Explorer (will prompt for admin)

**Note:** IDE terminals (VS Code, etc.) usually don't have admin rights. Use a separate PowerShell window.
:::

### Windows: Still showing old version after install?

If `rbxsync version` shows an old version after running the installer:

1. **Restart your terminal** - PATH changes don't take effect until you open a new terminal window
2. **Check all locations:** `where.exe rbxsync` - should show only one path
3. **If multiple locations exist**, delete the old one (see above)
4. **If old version is in System32**, you need admin rights to delete it (see warning above)

::: tip IDE terminals
If you're using the terminal inside VS Code or another IDE, close and reopen the entire IDE - not just the terminal tab.
:::

## Sync Issues

### Changes not syncing to Studio

1. **Check connection status** - The plugin widget should show green "Connected"
2. **Verify the path** - Make sure the project path in the plugin matches your actual project folder
3. **Restart the server** - Run `rbxsync stop` then `rbxsync serve`
4. **Check HttpService** - In Studio: Game Settings → Security → Allow HTTP Requests

### Script content not updating

If the plugin says "syncCreate success" but the script content doesn't change:

1. **Check for name mismatches** - The file name should match the instance name in Studio
2. **Try a full sync** - Run `rbxsync sync` from the CLI
3. **Delete and recreate** - Sometimes deleting the script in Studio and syncing again helps

### "Parent not found" errors

This usually means the parent folder doesn't exist in Studio. Make sure:
- The full path hierarchy exists in Studio
- Service names match (e.g., `ReplicatedStorage` not `replicatedstorage`)

## Plugin Issues

### Plugin not showing in Studio

1. **Restart Studio** - Always restart after installing/updating the plugin
2. **Check plugin folder** - Verify `RbxSync.rbxm` exists in:
   - macOS: `~/Documents/Roblox/Plugins/`
   - Windows: `%LOCALAPPDATA%\Roblox\Plugins\`
3. **Rebuild the plugin** - Run `rbxsync build-plugin --install`

### Plugin widget not appearing

1. Go to **View** menu in Studio
2. Look for **RbxSync** in the plugin widgets
3. Click to enable it

### "HttpService is not allowed" error

1. Open Studio's **Game Settings**
2. Go to **Security**
3. Enable **Allow HTTP Requests**

## Build Issues

### "Unknown property type" errors

Run `rbxsync fmt-project` to fix JSON formatting issues, or check the `.rbxjson` file for typos in property types.

### Build produces empty file

Make sure you have a valid `rbxsync.json` in your project root with the correct structure:
```json
{
  "name": "MyGame",
  "tree": {
    "$path": "src"
  }
}
```

## Performance

### Server using high CPU

The file watcher may be monitoring too many files. Add unnecessary directories to `.gitignore` or create a `.rbxsyncignore` file.

### Sync is slow for large games

Large games with many instances take longer to sync. Consider:
- Using selective sync for specific folders
- Breaking up large services into smaller modules

## Updating

### How do I update RbxSync?

```bash
rbxsync update
```

This pulls the latest code, rebuilds the CLI, and reinstalls the plugin. Remember to restart Studio after updating.

### How do I update the VS Code extension?

```bash
rbxsync update --vscode
code --install-extension rbxsync-vscode/rbxsync-*.vsix
```

Then restart VS Code.

## Still having issues?

1. Check the [Troubleshooting](/troubleshooting) guide
2. Run with debug logging: `RUST_LOG=debug rbxsync serve`
3. Join our Discord for help
4. [Open an issue on GitHub](https://github.com/devmarissa/rbxsync/issues)
