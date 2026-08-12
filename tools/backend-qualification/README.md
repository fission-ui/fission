# Backend qualification harness

This tool implements the fail-closed Phase 0/5 evidence contract from
`docs/rfc-multi-backend.md`. It does not contain benchmark results or numeric
budgets, and the generated template cannot qualify a backend until reviewed
values and real run evidence are supplied.

`qualification-manifest.json` is the checked-in matrix authority. Its required
target, browser, profile, and workload axes are frozen, but its environment,
input, build, toolchain, artifact ID, artifact SHA-256, and budget values are
deliberately null until the corresponding baselines and product ceilings have
been reviewed. It is therefore a checked-in qualification blocker, not a
production-qualification claim.

Generate the complete minimum matrix:

```sh
python3 tools/backend-qualification/qualification.py template \
  --output /absolute/path/to/qualification-manifest.json
```

The template requires:

- Linux, macOS, Windows, Android, and iOS native lanes;
- Chromium, Firefox, and WebKit interactive Web lanes;
- Vello-only, Skia-only, both-backend diagnostic, standalone-software, and
  Skia-plus-3D profiles. The standalone-software profile is Skia raster on
  native targets and CanvasKit's software surface on Web; it does not use the
  legacy Fission software renderer;
- application, widget, text, accessibility/IME, rendering, resource,
  mobile lifecycle, recovery, readback, video, web-content, external-surface,
  and 3D workloads.

Before collecting evidence, populate and review:

- `budget_revision`;
- every target's exact device/driver `environment_id`;
- every workload's input ID, binding the application/scene, fonts, assets, and
  benchmark content used by every backend;
- every target/profile build, toolchain, artifact ID, and exact artifact-byte
  SHA-256;
- every target-specific numeric budget.

No default numbers are supplied. A missing value remains an explicit blocker.
Additional target/device variants may be added, but removing or changing any
required lane, browser, profile, or workload is rejected.

Check whether the manifest is ready for evidence collection:

```sh
python3 tools/backend-qualification/qualification.py check-manifest \
  --manifest tools/backend-qualification/qualification-manifest.json \
  --json-output /absolute/path/to/manifest-readiness.json
```

This command exits `1` and records every missing value until the full matrix has
reviewed, explicit identities and target-specific numeric ceilings. Structural
or JSON failures exit `2`. It never fills a budget from a default or from a test
fixture.

Each `--evidence` file describes one target/profile run. It must use the frozen
identity and contain all workloads. Every workload records four sample arrays,
four memory values, and semantic and visual suite outcomes:

```json
{
  "schema_version": 2,
  "matrix_revision": "rfc-multi-backend-phase-0-v1",
  "run_id": "run-linux-skia-001",
  "identity": {
    "target_id": "linux-x86_64-gnu",
    "profile_id": "skia-only",
    "environment_id": "linux-device-driver-v1",
    "backend_ids": ["fission-render-skia"],
    "build_id": "build-id",
    "toolchain_id": "toolchain-id",
    "artifact_id": "artifact-id",
    "artifact_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  },
  "workflows": {
    "build": {"status": "pass", "evidence_id": "build-log-id"}
  },
  "size_bytes": {
    "native_raw": 0,
    "native_compressed": 0,
    "web_raw": null,
    "web_compressed": null
  },
  "workloads": {
    "scroll": {
      "input_id": "scroll-input-v1",
      "samples_ms": {
        "startup_ms": [0],
        "frame_ms": [0],
        "input_ms": [0],
        "recovery_ms": [0]
      },
      "memory_bytes": {
        "cold_bytes": 0,
        "warm_bytes": 0,
        "peak_bytes": 0,
        "gpu_bytes": 0
      },
      "suites": {
        "semantic": {"status": "pass", "evidence_id": "semantic-id"},
        "visual": {"status": "pass", "evidence_id": "visual-id"}
      }
    }
  }
}
```

The abbreviated example shows field shapes only; zero is not a recommended
budget or measurement, and a real file must include every workflow and
workload named by the manifest. `artifact_sha256` is exactly 64 lowercase
hexadecimal characters. It identifies the bytes that were exercised, not merely
an artifact name or release label, and evidence must match the digest frozen for
its target/profile cell exactly. A null digest is permitted only in the
deliberately unready frozen manifest and remains a readiness blocker.

Generate deterministic machine and human reports:

```sh
python3 tools/backend-qualification/qualification.py report \
  --manifest /absolute/path/to/qualification-manifest.json \
  --evidence /absolute/path/to/run-linux-skia.json \
  --json-output /absolute/path/to/report.json \
  --markdown-output /absolute/path/to/report.md
```

Percentiles use deterministic linear interpolation over sorted samples. The
startup timer must include process start through first-contentful frame. The
report command exits `0` only for a complete passing matrix, `1` for an
unqualified result, and `2` for invalid JSON or a structurally invalid frozen
manifest. The
both-backend diagnostic profile still requires measurements and passing
functional evidence, but its duplicate memory and size are recorded rather
than used for single-backend budget comparison.
