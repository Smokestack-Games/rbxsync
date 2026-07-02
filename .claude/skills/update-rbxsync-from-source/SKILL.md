---
name: update-rbxsync-from-source
description: Build and deploy rbxsync from the local source tree — Studio plugin plus CLI/server binary. Use when Ben says "update my rbxsync from source" (or asks to install/deploy the plugin and server from the repo).
---

# Update rbxsync from source

Builds the current checkout and deploys both halves of rbxsync locally: the Studio plugin to the Roblox plugins folder, and the CLI/server binary to `~/.cargo/bin`. Run from the repo root (`C:\Users\bnjmn\Documents\Git\rbxsync`).

## Steps

1. **Preflight.** Confirm the working tree state is what Ben expects to ship:
   - `git log --oneline -1` and `git status --short` — report branch/commit; warn if the tree is dirty (uncommitted changes will be baked into the binaries).
   - `curl -s -m 2 http://localhost:44755/health` — note whether a server is currently running (it will keep running the OLD binary until restarted).

2. **Build + install the Studio plugin:**
   ```
   cargo run -p rbxsync -- build-plugin --install
   ```
   Expected tail: `✓ Plugin installed: C:\Users\bnjmn\AppData\Local\Roblox\Plugins\RbxSync.rbxm`. (Package name is `rbxsync`, not `rbxsync-cli`.)

3. **Install the CLI/server binary (release mode, ~1-2 min):**
   ```
   cargo install --path rbxsync-cli
   ```
   Expected tail: `Replacing C:\Users\bnjmn\.cargo\bin\rbxsync.exe`.

4. **Verify the deployed binary.** Only if nothing was listening on 44755 in step 1: boot it briefly and probe, then stop it:
   ```
   rbxsync serve --port 44755 &   # temp check server
   curl -s http://localhost:44755/health
   curl -s "http://localhost:44755/snapshot/status?projectDir=C:/nonexistent"
   # kill the temp server afterward
   ```
   Both endpoints must answer (`/snapshot/status` returns `{"baseline":false,...}` for a nonexistent path). Also confirm the timestamps are fresh: `~/.cargo/bin/rbxsync.exe` and `~/AppData/Local/Roblox/Plugins/RbxSync.rbxm`.

5. **Report and remind:**
   - Plugin + binary timestamps and the commit they were built from.
   - Restart Roblox Studio to load the new plugin.
   - If a server was running in step 1: it is still the old binary — offer `rbxsync stop` and remind Ben to re-run `rbxsync serve` in his project folder (do NOT kill his running server without asking).

## Notes

- Debug artifacts in `target/debug` are NOT the deployed server; only `cargo install` updates the PATH binary.
- If `cargo install` fails on a file lock, a running `rbxsync.exe` is holding the binary — stop it first (with Ben's OK).
