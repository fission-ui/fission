This directory contains a vendored copy of Google Material icons used by this
repo. It avoids requiring the full material-design-icons submodule for builds.

Layout:
- `unique/` stores deduplicated SVGs (content-hash filenames).
- `index.json` maps category/icon/variant -> unique SVG path.

To refresh or add icons:
1) Run `./crates/authoring/fission-icons/tools/fetch_material_icons.sh` (sparse clone).
2) Run `python3 crates/authoring/fission-icons/tools/vendor_material_icons.py`.
3) Rebuild to regenerate the Rust include list.
