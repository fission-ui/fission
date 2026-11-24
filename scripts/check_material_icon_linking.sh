#!/usr/bin/env bash
# Check the actual release executable, not merely the generator's source shape.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --locked --release -p fission-icons --example one_icon

python3 - <<'PY'
import json
import os
from pathlib import Path
import subprocess

target = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
if os.environ.get("CARGO_BUILD_TARGET"):
    target /= os.environ["CARGO_BUILD_TARGET"]
binary = target / "release/examples/one_icon"
if os.name == "nt":
    binary = binary.with_suffix(".exe")
vendor = Path("crates/authoring/fission-icons")
index = json.loads((vendor / "material-icons-vendor/index.json").read_text())
used = (vendor / index["navigation"]["close"]["regular"]).read_bytes()
unused = (vendor / index["action"]["home"]["regular"]).read_bytes()
assert unused not in used, "the unused-icon fixture must differ from the used SVG"
assert subprocess.check_output([str(binary.resolve())]) == used, "icon accessor changed its SVG"
contents = binary.read_bytes()
assert used in contents, "the fixture must retain its selected icon"
assert unused not in contents, "one icon retained the unused catalogue in the release binary"
print(f"Single-icon executable: {len(contents):,} bytes; unused SVG is absent")
PY
