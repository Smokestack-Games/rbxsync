#!/usr/bin/env python3
"""
Write extracted game data to organized file tree structure.

Structure:
  src/
  ├── Workspace/
  │   ├── _meta.rbxjson          # Service properties
  │   ├── Model/
  │   │   ├── _meta.rbxjson      # Model properties
  │   │   └── Part.rbxjson       # Leaf instance
  │   └── Script.server.luau     # Script source
  ├── ReplicatedStorage/
  │   └── ...
  └── ...
"""

import json
import os
import sys
from pathlib import Path
from collections import defaultdict
from typing import Dict, List, Any, Optional

# Services to extract (top-level containers)
SERVICES = {
    "Workspace",
    "ReplicatedStorage",
    "ReplicatedFirst",
    "ServerScriptService",
    "ServerStorage",
    "StarterGui",
    "StarterPack",
    "StarterPlayer",
    "Lighting",
    "SoundService",
    "Teams",
    "Chat",
    "LocalizationService",
    "TestService",
}

def sanitize_filename(name: str) -> str:
    """Make a string safe for use as a filename."""
    # Replace problematic characters
    unsafe = '<>:"/\\|?*'
    for char in unsafe:
        name = name.replace(char, '_')
    # Limit length
    if len(name) > 200:
        name = name[:200]
    return name or "unnamed"

def get_script_extension(class_name: str) -> str:
    """Get the appropriate file extension for a script class."""
    if class_name == "Script":
        return ".server.luau"
    elif class_name == "LocalScript":
        return ".client.luau"
    elif class_name == "ModuleScript":
        return ".luau"
    return ".luau"

def build_tree(instances: List[Dict]) -> Dict[str, Any]:
    """Build a tree structure from flat instance list."""
    # Index by referenceId
    by_id = {inst["referenceId"]: inst for inst in instances if "referenceId" in inst}

    # Build parent->children mapping
    children_map = defaultdict(list)
    roots = []

    for inst in instances:
        parent_id = inst.get("parentId")
        if parent_id and parent_id in by_id:
            children_map[parent_id].append(inst)
        else:
            # Check if it's a service (root level)
            path = inst.get("path", "")
            if path and "." not in path and path in SERVICES:
                roots.append(inst)

    return {
        "by_id": by_id,
        "children_map": children_map,
        "roots": roots
    }

def instance_to_rbxjson(inst: Dict) -> Dict:
    """Convert instance to .rbxjson format."""
    result = {
        "className": inst.get("className"),
        "name": inst.get("name"),
        "referenceId": inst.get("referenceId"),
    }

    # Add properties (excluding some internal ones)
    if "properties" in inst:
        result["properties"] = inst["properties"]

    # Add attributes if present
    if "attributes" in inst:
        result["attributes"] = inst["attributes"]

    # Add tags if present
    if "tags" in inst:
        result["tags"] = inst["tags"]

    return result

def write_instance(
    inst: Dict,
    tree: Dict,
    output_dir: Path,
    depth: int = 0,
    stats: Dict = None
) -> None:
    """Write an instance and its children to the file tree."""
    if stats is None:
        stats = {"files": 0, "dirs": 0, "scripts": 0}

    inst_id = inst.get("referenceId")
    class_name = inst.get("className", "Unknown")
    name = sanitize_filename(inst.get("name", "unnamed"))
    children = tree["children_map"].get(inst_id, [])

    # Determine if this instance has children (is a container)
    is_container = len(children) > 0

    # Handle scripts specially
    is_script = class_name in ("Script", "LocalScript", "ModuleScript")

    if is_script:
        # Write script source to .luau file
        source = inst.get("source", "")
        if source:
            ext = get_script_extension(class_name)
            script_path = output_dir / f"{name}{ext}"

            # Handle name collisions
            counter = 1
            while script_path.exists():
                script_path = output_dir / f"{name}_{counter}{ext}"
                counter += 1

            with open(script_path, "w", encoding="utf-8") as f:
                f.write(source)
            stats["scripts"] += 1

        # Also write properties to .rbxjson
        props_path = output_dir / f"{name}.rbxjson"
        counter = 1
        while props_path.exists():
            props_path = output_dir / f"{name}_{counter}.rbxjson"
            counter += 1

        rbxjson = instance_to_rbxjson(inst)
        # Remove source from properties (it's in the .luau file)
        if "source" in rbxjson:
            del rbxjson["source"]

        with open(props_path, "w", encoding="utf-8") as f:
            json.dump(rbxjson, f, indent=2)
        stats["files"] += 1

        # Scripts can have children too (e.g., ModuleScripts inside Scripts)
        if children:
            child_dir = output_dir / name
            child_dir.mkdir(exist_ok=True)
            stats["dirs"] += 1
            for child in children:
                write_instance(child, tree, child_dir, depth + 1, stats)

    elif is_container:
        # Create directory for container
        container_dir = output_dir / name
        container_dir.mkdir(exist_ok=True)
        stats["dirs"] += 1

        # Write container's own properties to _meta.rbxjson
        meta_path = container_dir / "_meta.rbxjson"
        rbxjson = instance_to_rbxjson(inst)
        with open(meta_path, "w", encoding="utf-8") as f:
            json.dump(rbxjson, f, indent=2)
        stats["files"] += 1

        # Write children
        for child in children:
            write_instance(child, tree, container_dir, depth + 1, stats)

    else:
        # Leaf instance - write as single .rbxjson file
        file_path = output_dir / f"{name}.rbxjson"

        # Handle name collisions
        counter = 1
        while file_path.exists():
            file_path = output_dir / f"{name}_{counter}.rbxjson"
            counter += 1

        rbxjson = instance_to_rbxjson(inst)
        with open(file_path, "w", encoding="utf-8") as f:
            json.dump(rbxjson, f, indent=2)
        stats["files"] += 1

def write_file_tree(input_path: str, output_dir: str) -> None:
    """Convert extracted game data to file tree structure."""
    input_file = Path(input_path)
    output_path = Path(output_dir)

    print(f"Reading {input_file}...")
    with open(input_file) as f:
        data = json.load(f)

    instances = data.get("instances", [])
    print(f"Loaded {len(instances)} instances")

    # Build tree structure
    print("Building tree structure...")
    tree = build_tree(instances)
    print(f"Found {len(tree['roots'])} root services")

    # Create output directory
    src_dir = output_path / "src"
    src_dir.mkdir(parents=True, exist_ok=True)

    # Write each service
    stats = {"files": 0, "dirs": 0, "scripts": 0}

    for root in tree["roots"]:
        service_name = root.get("name", "Unknown")
        print(f"Writing {service_name}...")
        write_instance(root, tree, src_dir, stats=stats)

    print(f"\nDone!")
    print(f"  Directories: {stats['dirs']}")
    print(f"  .rbxjson files: {stats['files']}")
    print(f"  Script files: {stats['scripts']}")
    print(f"\nOutput: {output_path}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        input_path = Path.home() / "rbxsync" / "output_v2" / "game_data.json"
    else:
        input_path = sys.argv[1]

    output_dir = sys.argv[2] if len(sys.argv) > 2 else str(Path.home() / "rbxsync" / "game_project")

    write_file_tree(str(input_path), output_dir)
