#!/usr/bin/env python3
"""
Sync file tree to Roblox Studio.

Reads .rbxjson files and script files from the project directory
and pushes them to Studio via the RbxSync server.
"""

import json
import os
import sys
import urllib.request
import urllib.error
from pathlib import Path
from typing import Dict, List, Any, Optional

SERVER_URL = "http://localhost:44755"

def http_get(url: str, timeout: int = 5) -> Dict:
    """Make HTTP GET request."""
    req = urllib.request.Request(url)
    with urllib.request.urlopen(req, timeout=timeout) as response:
        return json.loads(response.read().decode())

def http_post(url: str, data: Dict, timeout: int = 30) -> tuple:
    """Make HTTP POST request. Returns (status_code, response_dict)."""
    json_data = json.dumps(data).encode('utf-8')
    req = urllib.request.Request(url, data=json_data, method='POST')
    req.add_header('Content-Type', 'application/json')
    try:
        with urllib.request.urlopen(req, timeout=timeout) as response:
            return (response.status, json.loads(response.read().decode()))
    except urllib.error.HTTPError as e:
        try:
            body = json.loads(e.read().decode())
        except:
            body = {"error": str(e)}
        return (e.code, body)

def read_rbxjson(file_path: Path) -> Optional[Dict]:
    """Read and parse a .rbxjson file."""
    try:
        with open(file_path, encoding="utf-8") as f:
            return json.load(f)
    except Exception as e:
        print(f"Warning: Could not read {file_path}: {e}")
        return None

def read_script_source(script_path: Path) -> Optional[str]:
    """Read script source from a .luau file."""
    try:
        with open(script_path, encoding="utf-8") as f:
            return f.read()
    except Exception as e:
        print(f"Warning: Could not read {script_path}: {e}")
        return None

def get_instance_path(file_path: Path, src_dir: Path) -> str:
    """Convert file path to Roblox instance path."""
    # Get relative path from src directory
    rel_path = file_path.relative_to(src_dir)

    # Convert to instance path
    parts = list(rel_path.parts)

    # Handle special files
    if parts[-1] == "_meta.rbxjson":
        # Container metadata - path is the directory
        parts = parts[:-1]
    elif parts[-1].endswith(".rbxjson"):
        # Instance file - remove extension
        parts[-1] = parts[-1][:-8]  # Remove .rbxjson
    elif parts[-1].endswith(".server.luau"):
        parts[-1] = parts[-1][:-12]  # Remove .server.luau
    elif parts[-1].endswith(".client.luau"):
        parts[-1] = parts[-1][:-12]  # Remove .client.luau
    elif parts[-1].endswith(".luau"):
        parts[-1] = parts[-1][:-5]  # Remove .luau

    return ".".join(parts)

def collect_operations(src_dir: Path, limit: int = 0, service_filter: Optional[str] = None) -> List[Dict]:
    """Collect all sync operations from the file tree."""
    operations = []
    processed_paths = set()
    file_count = 0

    # Determine which directory to scan
    scan_dir = src_dir
    if service_filter:
        scan_dir = src_dir / service_filter
        if not scan_dir.exists():
            print(f"Service not found: {service_filter}")
            return []

    # Walk the directory tree
    for root, dirs, files in os.walk(scan_dir):
        root_path = Path(root)

        for file in files:
            file_count += 1
            if file_count % 1000 == 0:
                print(f"  Scanned {file_count} files, found {len(operations)} operations...")

            # Check limit
            if limit > 0 and len(operations) >= limit:
                print(f"  Reached limit of {limit} operations")
                return operations
            file_path = root_path / file

            # Skip non-relevant files
            if not (file.endswith(".rbxjson") or file.endswith(".luau")):
                continue

            instance_path = get_instance_path(file_path, src_dir)

            # Skip if already processed (e.g., script.luau and script.rbxjson)
            if instance_path in processed_paths:
                # But update with source if this is the .luau file
                if file.endswith(".luau"):
                    source = read_script_source(file_path)
                    if source:
                        # Find and update the existing operation
                        for op in operations:
                            if op.get("path") == instance_path:
                                if op.get("data"):
                                    op["data"]["source"] = source
                                break
                continue

            processed_paths.add(instance_path)

            # Read data
            if file.endswith(".rbxjson"):
                data = read_rbxjson(file_path)
                if not data:
                    continue

                # Check for corresponding .luau file
                for ext in [".server.luau", ".client.luau", ".luau"]:
                    luau_path = file_path.parent / (file_path.stem + ext)
                    if luau_path.exists():
                        source = read_script_source(luau_path)
                        if source:
                            data["source"] = source
                        break

                operations.append({
                    "type": "update",
                    "path": instance_path,
                    "data": data
                })

            elif file.endswith(".luau"):
                # Script file without .rbxjson - create minimal data
                source = read_script_source(file_path)
                if not source:
                    continue

                # Determine script type from extension
                if file.endswith(".server.luau"):
                    class_name = "Script"
                    name = file[:-12]
                elif file.endswith(".client.luau"):
                    class_name = "LocalScript"
                    name = file[:-12]
                else:
                    class_name = "ModuleScript"
                    name = file[:-5]

                operations.append({
                    "type": "update",
                    "path": instance_path,
                    "data": {
                        "className": class_name,
                        "name": name,
                        "source": source
                    }
                })

    return operations

def sync_to_studio(src_dir: str, batch_size: int = 50, limit: int = 0, service: Optional[str] = None) -> None:
    """Sync file tree to Studio."""
    src_path = Path(src_dir)

    if not src_path.exists():
        print(f"Error: Source directory not found: {src_dir}")
        sys.exit(1)

    # Check server connection
    try:
        http_get(f"{SERVER_URL}/health", timeout=5)
    except Exception as e:
        print(f"Error: Cannot connect to RbxSync server. Is it running?")
        print("Start it with: rbxsync serve")
        sys.exit(1)

    print(f"Collecting operations from {src_path}...")
    if service:
        print(f"  Filtering to service: {service}")
    if limit > 0:
        print(f"  Limiting to {limit} operations")
    operations = collect_operations(src_path, limit=limit, service_filter=service)

    if not operations:
        print("No files to sync")
        return

    print(f"Found {len(operations)} instances to sync")

    # Sort operations to ensure parents are created before children
    # Key: (depth, path) - this ensures:
    # 1. Shallower paths come first (depth)
    # 2. Within same depth, alphabetical order ensures parent paths come before children
    #    because "Foo" < "Foo.Bar" lexicographically
    def sort_key(op):
        path = op.get("path", "")
        depth = path.count(".")
        return (depth, path)

    operations.sort(key=sort_key)

    # Collect all paths that have operations
    paths_with_ops = {op.get("path", "") for op in operations}

    # Add all service roots as "seen" since they always exist
    services = {
        "Workspace", "ReplicatedStorage", "ReplicatedFirst",
        "ServerScriptService", "ServerStorage", "StarterGui",
        "StarterPack", "StarterPlayer", "Lighting", "SoundService",
        "Teams", "Chat", "LocalizationService", "TestService"
    }

    # Find all missing parent paths and create Folder operations for them
    missing_parents = set()
    for op in operations:
        path = op.get("path", "")
        parts = path.split(".")
        # Check all ancestor paths
        for i in range(1, len(parts)):
            ancestor_path = ".".join(parts[:i])
            if ancestor_path not in paths_with_ops and ancestor_path not in services:
                missing_parents.add(ancestor_path)

    # Create Folder operations for missing parents
    if missing_parents:
        print(f"  Creating {len(missing_parents)} missing parent Folders")
        for parent_path in missing_parents:
            parts = parent_path.split(".")
            name = parts[-1] if parts else parent_path
            operations.append({
                "type": "update",
                "path": parent_path,
                "data": {
                    "className": "Folder",
                    "name": name
                }
            })
            paths_with_ops.add(parent_path)  # Mark as having an operation now

    # Now sort with topological ordering
    seen_paths = set(services)
    reordered = []
    pending = list(operations)

    max_iterations = len(pending) * 2  # Prevent infinite loop
    iterations = 0

    while pending and iterations < max_iterations:
        iterations += 1
        made_progress = False

        for op in pending[:]:  # Iterate over copy
            path = op.get("path", "")
            parts = path.split(".")

            # Check if parent exists (or this is a root service)
            if len(parts) <= 1:
                # Root level - always ok
                reordered.append(op)
                seen_paths.add(path)
                pending.remove(op)
                made_progress = True
            else:
                parent_path = ".".join(parts[:-1])
                if parent_path in seen_paths:
                    reordered.append(op)
                    seen_paths.add(path)
                    pending.remove(op)
                    made_progress = True

        if not made_progress:
            # No progress - remaining items have missing parents
            # Add them anyway and let the plugin handle failures
            break

    # Add any remaining items
    reordered.extend(pending)
    operations = reordered

    if pending:
        print(f"  Warning: {len(pending)} operations may have missing parents")

    # Send in batches
    total_batches = (len(operations) + batch_size - 1) // batch_size
    success_count = 0
    fail_count = 0

    for i in range(0, len(operations), batch_size):
        batch = operations[i:i + batch_size]
        batch_num = i // batch_size + 1

        print(f"Sending batch {batch_num}/{total_batches} ({len(batch)} operations)...")

        try:
            status_code, result = http_post(
                f"{SERVER_URL}/sync/batch",
                {"operations": batch},
                timeout=300  # 5 minute timeout
            )

            if status_code == 200:
                if result.get("success"):
                    success_count += len(batch)
                    print(f"  Batch {batch_num} succeeded")
                else:
                    # Count individual successes/failures
                    results = result.get("data", {}).get("results", [])
                    for r in results:
                        if r.get("success"):
                            success_count += 1
                        else:
                            fail_count += 1
                    print(f"  Batch {batch_num} partial: {sum(1 for r in results if r.get('success'))}/{len(batch)} succeeded")
            elif status_code == 504:
                print(f"  Batch {batch_num} timed out - plugin may not be connected")
                fail_count += len(batch)
            else:
                print(f"  Batch {batch_num} failed: HTTP {status_code}")
                fail_count += len(batch)

        except Exception as e:
            print(f"  Batch {batch_num} error: {e}")
            fail_count += len(batch)

    print(f"\nSync complete!")
    print(f"  Succeeded: {success_count}")
    print(f"  Failed: {fail_count}")

def sync_single_file(file_path: str) -> None:
    """Sync a single file to Studio."""
    path = Path(file_path)

    if not path.exists():
        print(f"Error: File not found: {file_path}")
        sys.exit(1)

    # Find the src directory
    src_dir = path
    while src_dir.name != "src" and src_dir.parent != src_dir:
        src_dir = src_dir.parent

    if src_dir.name != "src":
        print("Error: Could not find src directory in path")
        sys.exit(1)

    instance_path = get_instance_path(path, src_dir)

    # Read data
    if path.suffix == ".rbxjson":
        data = read_rbxjson(path)
    elif path.suffix == ".luau":
        source = read_script_source(path)
        if path.stem.endswith(".server"):
            class_name = "Script"
        elif path.stem.endswith(".client"):
            class_name = "LocalScript"
        else:
            class_name = "ModuleScript"
        data = {
            "className": class_name,
            "name": path.stem.split(".")[0],
            "source": source
        }
    else:
        print(f"Error: Unknown file type: {path.suffix}")
        sys.exit(1)

    if not data:
        print("Error: Could not read file data")
        sys.exit(1)

    print(f"Syncing {instance_path}...")

    try:
        status_code, result = http_post(
            f"{SERVER_URL}/sync/command",
            {
                "command": "sync:update",
                "payload": {
                    "path": instance_path,
                    "data": data
                }
            },
            timeout=30
        )

        if status_code == 200:
            if result.get("success"):
                print("Sync succeeded!")
            else:
                print(f"Sync failed: {result.get('error', 'Unknown error')}")
        else:
            print(f"Sync failed: HTTP {status_code}")

    except Exception as e:
        print(f"Sync error: {e}")

if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Sync file tree to Roblox Studio")
    parser.add_argument("path", nargs="?", default=str(Path.home() / "rbxsync" / "game_project" / "src"),
                        help="Source directory or file to sync")
    parser.add_argument("--batch-size", "-b", type=int, default=50,
                        help="Batch size for operations (default: 50)")
    parser.add_argument("--limit", "-l", type=int, default=0,
                        help="Limit number of operations (0 = no limit)")
    parser.add_argument("--service", "-s", type=str, default=None,
                        help="Only sync specific service (e.g., ServerScriptService)")

    args = parser.parse_args()

    # Check if it's a single file or directory
    if Path(args.path).is_file():
        sync_single_file(args.path)
    else:
        sync_to_studio(args.path, args.batch_size, args.limit, args.service)
