#!/usr/bin/env python3
"""
Consolidate extracted chunks into a single organized structure.
"""

import json
import os
import sys
from pathlib import Path
from collections import defaultdict

def consolidate_extraction(extract_dir: str, output_dir: str):
    """Consolidate all chunks into organized output."""

    extract_path = Path(extract_dir)
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    # Read all chunks
    all_instances = []
    chunk_files = sorted(extract_path.glob("chunk_*.json"))

    print(f"Reading {len(chunk_files)} chunks...")

    for i, chunk_file in enumerate(chunk_files):
        with open(chunk_file) as f:
            try:
                chunk_data = json.load(f)
                if isinstance(chunk_data, list):
                    all_instances.extend(chunk_data)
            except json.JSONDecodeError as e:
                print(f"Warning: Failed to parse {chunk_file}: {e}")

        if (i + 1) % 1000 == 0:
            print(f"  Read {i + 1}/{len(chunk_files)} chunks...")

    print(f"Total instances: {len(all_instances)}")

    # Build index by referenceId
    instances_by_id = {}
    for inst in all_instances:
        ref_id = inst.get("referenceId")
        if ref_id:
            instances_by_id[ref_id] = inst

    # Group instances by className
    by_class = defaultdict(list)
    for inst in all_instances:
        by_class[inst.get("className", "Unknown")].append(inst)

    # Print statistics
    print("\nClass distribution (top 20):")
    sorted_classes = sorted(by_class.items(), key=lambda x: -len(x[1]))
    for class_name, instances in sorted_classes[:20]:
        print(f"  {class_name}: {len(instances)}")

    # Save combined data
    print(f"\nWriting combined output to {output_path}...")

    # Save all instances to a single file
    combined_output = {
        "totalInstances": len(all_instances),
        "classCount": len(by_class),
        "instances": all_instances
    }

    with open(output_path / "game_data.json", "w") as f:
        json.dump(combined_output, f, separators=(',', ':'))

    print(f"Wrote game_data.json ({os.path.getsize(output_path / 'game_data.json') / 1024 / 1024:.1f} MB)")

    # Save class index
    class_index = {k: len(v) for k, v in by_class.items()}
    with open(output_path / "class_index.json", "w") as f:
        json.dump(class_index, f, indent=2)

    # Extract scripts to separate files
    scripts_dir = output_path / "scripts"
    scripts_dir.mkdir(exist_ok=True)

    script_count = 0
    for inst in all_instances:
        if inst.get("source"):
            class_name = inst.get("className", "Script")
            name = inst.get("name", "unnamed")
            # Create safe filename
            safe_name = "".join(c if c.isalnum() or c in "._-" else "_" for c in name)
            ext = ".server.luau" if class_name == "Script" else ".client.luau" if class_name == "LocalScript" else ".luau"

            script_path = scripts_dir / f"{safe_name}{ext}"
            # Handle duplicates
            counter = 1
            while script_path.exists():
                script_path = scripts_dir / f"{safe_name}_{counter}{ext}"
                counter += 1

            with open(script_path, "w") as f:
                f.write(inst["source"])
            script_count += 1

    print(f"Extracted {script_count} scripts to {scripts_dir}")

    print("\nDone!")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        # Find most recent extraction
        rbxsync_dir = Path.home() / "rbxsync" / ".rbxsync"
        extract_dirs = list(rbxsync_dir.glob("extract_*"))
        if not extract_dirs:
            print("No extraction directories found")
            sys.exit(1)
        extract_dir = max(extract_dirs, key=lambda p: p.stat().st_mtime)
        print(f"Using most recent extraction: {extract_dir.name}")
    else:
        extract_dir = sys.argv[1]

    output_dir = sys.argv[2] if len(sys.argv) > 2 else str(Path.home() / "rbxsync" / "output")

    consolidate_extraction(str(extract_dir), output_dir)
