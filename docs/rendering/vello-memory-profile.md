# Vello memory profile evidence

Date: 2026-06-29
Host: macOS 26.1 on Apple M1 Pro
Fission build mode: `cargo build --release`
Measurement tool: `vmmap -summary <pid>` physical footprint unless otherwise stated.

## Baseline before the Vello fork

These measurements used the crates.io Vello 0.6.0 dependency and the current Fission GPU path.

| Case | Renderer / mode | Physical footprint | Notes |
| --- | --- | ---: | --- |
| `examples/inbox` | default Metal/Vello | 214.4 MiB | Simple inbox screen, 800x600 default window. |
| `examples/inbox` | `FISSION_RENDERER=software` | 57.5 MiB | Same app without Vello GPU path. |
| `examples/counter` | default Metal/Vello | 247.3 MiB | Minimal app still paid the Vello fixed dynamic-buffer cost. |
| `examples/motion-memory-repro` | default Metal/Vello | 262.9 MiB | Plain/default repro path. |
| Minimal Vello two-rect probe | after renderer creation | ~17 MiB | Vello initialized, no render submitted. |
| Minimal Vello two-rect probe | after first `render_to_texture` | ~192 MiB | Proves the large allocation comes from Vello render-time buffers, not Fission display-list complexity. |

## Root cause found in Vello 0.6.0 / 0.9.0

`vello_encoding/src/config.rs::BufferSizes::new` hard-coded bump-allocated dynamic buffers for every render:

| Buffer | Elements | Approx bytes |
| --- | ---: | ---: |
| `bin_data` | `1 << 18` u32 | 1 MiB |
| `tiles` | `1 << 21` `Tile` | 16 MiB |
| `lines` | `1 << 21` `LineSoup` | 48 MiB |
| `seg_counts` | `1 << 21` `SegmentCount` | 16 MiB |
| `segments` | `1 << 21` `PathSegment` | 48 MiB |
| `blend_spill` | `1 << 20` u32 | 4 MiB |
| `ptcl` | `1 << 23` u32 | 32 MiB |

Total fixed dynamic working set: about 165 MiB before scene complexity matters.

## First Worka Vello fork experiment

Fork/branch: `worka-ai/vello`, branch `worka/dynamic-gpu-buffers`, commit `8ee23dc2`.
Change: replaced fixed dynamic-buffer constants with initial scene/viewport-derived sizing. This proved the memory source but had no grow/retry validation.

| Case | Physical footprint | Peak | Notes |
| --- | ---: | ---: | --- |
| `examples/inbox` | 65.2 MiB | 79.0 MiB | One-shot 800x600 launch. |
| `examples/inbox` | 67-68 MiB | n/a | Stable across 60s at 800x600. |
| `examples/counter` | 52.5 MiB | 66.1 MiB | One-shot 800x600 launch. |
| `examples/motion-memory-repro` | 88.3 MiB | 106.5 MiB | One-shot 800x600 launch. |

Resize test using Fission test-control screenshots on the same process:

| Logical viewport | Physical footprint after screenshot | Notes |
| --- | ---: | --- |
| 800x600 | 186.5 MiB | Includes screenshot/readback path and pooled resources. |
| 1200x900 | 196.9 MiB | Same process after resize. |
| 1600x1200 | 195.0 MiB | Same process after resize. |
| 2400x1800 | 662.6 MiB | Large viewport memory spike. |
| 3200x2400 | 1.1 GiB | Large viewport memory spike; manual testing showed grey output when too large. |

## Required upstreamable fix shape

The fork should not guess from fixed constants. It should support:

1. Caller-provided workload profile for precise initial allocation.
2. Vello layout/workgroup refinement.
3. GPU bump-counter validation on every render.
4. Grow/retry when any bump counter exceeds its allocation.
5. Diagnostics exposing requested sizes, actual bump usage, retry count, and wasted bytes.

This keeps the API generic: callers provide renderer workload facts, not Fission-specific widget state.

## Profiled dynamic-buffer fork result

Change set:

- Added `RenderWorkloadProfile`/`DynamicBufferPolicy` to Vello so callers can provide target size, tile coverage, scene complexity, and sizing policy.
- Added Vello grow/retry validation by reading back the GPU bump counters after the coarse pass.
- Added bounds checks for `ptcl` and `segments` writes in the coarse shader so failed allocations are reported instead of producing undefined output.
- Added retry-counter sanitising so counters from stages after an earlier failed stage are not blindly trusted.
- Changed Fission to calculate the workload profile while walking its retained display list and pass it to Vello for each render.

Validated release-build measurements after the profiled dynamic-buffer integration:

| Case | Physical footprint | Peak | Notes |
| --- | ---: | ---: | --- |
| `examples/inbox` | 50.0 MiB | 64.6 MiB | 800x600 one-shot launch. |
| `examples/counter` | 42.4 MiB | 56.5 MiB | 800x600 one-shot launch. |
| `examples/motion-memory-repro` | 74.1 MiB | 88.7 MiB | 800x600 one-shot launch. |

`examples/inbox` stability over a single 800x600 run:

| Elapsed | Physical footprint | Peak |
| --- | ---: | ---: |
| 5s | 49.3 MiB | 63.6 MiB |
| 15s | 49.3 MiB | 63.6 MiB |
| 30s | 47.6 MiB | 63.6 MiB |
| 60s | 47.6 MiB | 63.6 MiB |

Resize test using Fission test-control screenshots after the final profile/grow path:

| Logical viewport | Physical footprint after screenshot | Peak | Screenshot |
| --- | ---: | ---: | --- |
| 800x600 | 61.3 MiB | 66.4 MiB | `/tmp/fission-inbox-profile-shots-1600/800x600.png` |
| 1200x900 | 95.8 MiB | 104.1 MiB | `/tmp/fission-inbox-profile-shots-1600/1200x900.png` |
| 1600x1200 | 113.9 MiB | 125.3 MiB | `/tmp/fission-inbox-profile-shots-1600/1600x1200.png` |

Screenshot sanity checks with ImageMagick reported non-flat images (`colors` = 917, 1393, 1335 respectively), and manual inspection of the 1600x1200 screenshot showed the inbox UI rendered rather than a grey/blank frame.

The 2400x1800 logical resize maps to a 4800x3600 physical render target on this display. It no longer allocates the original fixed 165 MiB for ordinary scenes, but it still exposes a Vello coarse-pass high-DPI edge case around repeated/inverted tile segment state. Vello now reports `DynamicBufferAllocationFailed` instead of silently producing grey output or unbounded allocation; the current Fission shell still panics because it expects render success. A full upstream fix likely needs Vello to preserve segment counts separately from allocated segment offsets or split very large render targets into bounded tiles.
