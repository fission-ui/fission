#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path

VARIANT_MAP = {
    "materialicons": "regular",
    "materialiconsoutlined": "outlined",
    "materialiconsround": "round",
    "materialiconssharp": "sharp",
    "materialiconstwotone": "two_tone",
}


def hash_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    script_dir = Path(__file__).resolve().parent
    manifest_dir = script_dir.parent

    parser = argparse.ArgumentParser(description="Vendor Material Icons with dedupe.")
    parser.add_argument(
        "--src",
        default=str(manifest_dir / "material-design-icons" / "src"),
        help="Path to material-design-icons/src",
    )
    parser.add_argument(
        "--out",
        default=str(manifest_dir / "material-icons-vendor"),
        help="Output vendor directory",
    )
    args = parser.parse_args()

    src_root = Path(args.src)
    out_root = Path(args.out)
    unique_dir = out_root / "unique"
    index_path = out_root / "index.json"

    if not src_root.exists():
        print(f"Source path not found: {src_root}", file=sys.stderr)
        return 1

    if unique_dir.exists():
        shutil.rmtree(unique_dir)
    unique_dir.mkdir(parents=True, exist_ok=True)

    icons: dict[str, dict[str, dict[str, str]]] = {}
    unique_count = 0
    total_count = 0

    for variant_dir in src_root.glob("*/*/*"):
        if not variant_dir.is_dir():
            continue

        variant_folder = variant_dir.name
        variant_key = VARIANT_MAP.get(variant_folder)
        if not variant_key:
            continue

        icon_name = variant_dir.parent.name
        category = variant_dir.parent.parent.name
        svg_path = variant_dir / "24px.svg"
        if not svg_path.exists():
            continue

        data = svg_path.read_bytes()
        digest = hash_bytes(data)
        unique_path = unique_dir / f"{digest}.svg"
        if not unique_path.exists():
            unique_path.write_bytes(data)
            unique_count += 1
        total_count += 1

        rel_path = unique_path.relative_to(manifest_dir).as_posix()
        icons.setdefault(category, {}).setdefault(icon_name, {})[variant_key] = rel_path

    out_root.mkdir(parents=True, exist_ok=True)
    with index_path.open("w", encoding="utf-8") as f:
        json.dump(icons, f, indent=2, sort_keys=True)
        f.write("\n")

    print(f"Indexed {total_count} icons; wrote {unique_count} unique SVGs.")
    print(f"Wrote {index_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
