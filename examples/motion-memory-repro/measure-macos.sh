#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

target_dir="$(cargo metadata --format-version=1 --no-deps \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
bin="$target_dir/debug/motion-memory-repro"

cargo build -p motion-memory-repro >/dev/null

out="${1:-/tmp/motion-memory-repro.tsv}"
printf 'scenario\trenderer\trows\timage_count\tpixels\tcache\trss_kb\tphysical\tpeak\towned_unmapped\tioaccel_graphics\tmalloc_large\tmalloc_large_count\n' > "$out"

extract_summary_value() {
  local file="$1" label="$2"
  awk -v label="$label" '
    {
      line = $0
      sub(/^[[:space:]]+/, "", line)
      if (index(line, label) == 1) {
        rest = substr(line, length(label) + 1)
        sub(/^[[:space:]]+/, "", rest)
        split(rest, parts, /[[:space:]]+/)
        print parts[1]
        exit
      }
    }
  ' "$file"
}

extract_malloc_large_size() {
  awk '$1 == "MALLOC_LARGE" && $2 != "(empty)" { print $2; exit }' "$1"
}

extract_malloc_large_count() {
  awk '$1 == "MALLOC_LARGE" && $2 != "(empty)" { print $9; exit }' "$1"
}

run_case() {
  local scenario="$1" renderer="$2" rows="$3" image_count="$4" pixels="$5" cache="$6"
  local name="${scenario}-${renderer}-rows${rows}-images${image_count}-px${pixels}-cache${cache}"
  local vmmap_file="/tmp/motion-memory-repro-${name}.vmmap"
  local log_file="/tmp/motion-memory-repro-${name}.log"

  echo "running $name" >&2
  (
    export FISSION_REPRO_SCENARIO="$scenario"
    export FISSION_REPRO_ROWS="$rows"
    export FISSION_REPRO_IMAGE_COUNT="$image_count"
    export FISSION_REPRO_IMAGE_PIXELS="$pixels"
    if [[ "$renderer" != auto ]]; then export FISSION_RENDERER="$renderer"; else unset FISSION_RENDERER || true; fi
    if [[ "$cache" == yes ]]; then export FISSION_REPRO_CACHE_IMAGES=1; else unset FISSION_REPRO_CACHE_IMAGES || true; fi
    exec "$bin"
  ) >"$log_file" 2>&1 &

  local pid=$!
  sleep 25

  local rss_kb=""
  if kill -0 "$pid" 2>/dev/null; then
    rss_kb="$(ps -o rss= -p "$pid" | tr -d ' ')"
    vmmap -summary "$pid" >"$vmmap_file" 2>/dev/null || true
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  else
    wait "$pid" 2>/dev/null || true
    : >"$vmmap_file"
  fi

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$scenario" "$renderer" "$rows" "$image_count" "$pixels" "$cache" "$rss_kb" \
    "$(extract_summary_value "$vmmap_file" 'Physical footprint:')" \
    "$(extract_summary_value "$vmmap_file" 'Physical footprint (peak):')" \
    "$(extract_summary_value "$vmmap_file" 'owned unmapped memory')" \
    "$(extract_summary_value "$vmmap_file" 'IOAccelerator (graphics)')" \
    "$(extract_malloc_large_size "$vmmap_file")" \
    "$(extract_malloc_large_count "$vmmap_file")" >> "$out"
}

run_case plain auto 48 1 1024 no
run_case plain-images auto 6 6 1024 no
run_case plain-images auto 48 1 1024 no
run_case plain-images auto 48 48 512 no
run_case plain-images auto 48 48 1024 no
run_case plain-images auto 48 48 1024 yes
run_case motion-images auto 48 48 1024 no

cat "$out"
