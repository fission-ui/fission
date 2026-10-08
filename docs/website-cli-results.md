# Finite CLI command results

`init`, `add-target`, `build`, and `site build|check|routes` accept a local
`--json` flag. Each command executes once and returns the same typed outcome
for the human and JSON renderers. JSON stdout contains one
`fission.cli-result.v1` object followed by a newline. Human output is the default.

```sh
fission init my-app --name my-app --json
# Read data.instructions before changing the project.
fission add-target terminal web --project-dir my-app --json
fission build --target terminal --project-dir my-app --json
fission build --project-dir my-app --json
```

Generic setup and build commands retain all existing targets: Android, iOS,
Linux, macOS, Windows, Terminal, Web, Static site and SSR. Omitted build targets
retain the host desktop default. Release, variant and Web Cargo feature options
use the same validation and target preparation in both output modes. `init`
retains existing platform defaults and file-preservation rules. Its result lists
instruction files without selecting a subsequent target.

For a user-selected Static site:

```sh
fission add-target static-site --project-dir my-app --json
fission site routes --project-dir my-app --json
fission site check --project-dir my-app --json
fission build --target static-site --project-dir my-app --json
```

## Public reports and envelope

Types live in `fission_command_core::report`. Command authorities return
`report::Result<Data>`; neither accepts an output-format argument. The CLI
constructs `CommandResult` and renders the completed outcome. The envelope
contains `schema`, `action`, absolute `project_dir`, `selected_targets`, optional
observed `state`, and the tagged `outcome`. The state contains the project name,
configuration path and configured targets. Actions are `init`, `add_target`,
`build`, `site_build`, `site_check`, and `site_routes`.

Success has `data`; failure has `error`. `Data` contains verified setup files in
`artifacts`, instruction paths in `instructions`, target reports in `targets`,
and `next_steps`. Current commands leave `next_steps` empty. Each `TargetData`
contains `target`, `artifact_dir`, `planned_output_dir`, `artifacts` and `routes`.
A consumer should find the record for its intended target rather than assume a
fixed list position.

A representative result is:

```json
{
  "schema": "fission.cli-result.v1",
  "action": "build",
  "project_dir": "/work/my-app",
  "selected_targets": ["terminal"],
  "state": {
    "name": "my-app",
    "config_path": "/work/my-app/fission.toml",
    "configured_targets": ["linux", "macos", "terminal", "windows"]
  },
  "outcome": "success",
  "data": {
    "artifacts": [],
    "instructions": [],
    "next_steps": [],
    "targets": [{
      "target": "terminal",
      "artifact_dir": null,
      "planned_output_dir": null,
      "artifacts": ["/work/my-app/target/debug/my-app"],
      "routes": []
    }]
  }
}
```

Paths come from the command authority and filesystem checks. Desktop/Terminal
reports include the verified executable; Android includes the APK reported by
its existing script. iOS and SSR builds retain their existing execution paths
without guessing artifact locations. A successful target with empty artifact
fields means execution completed but no artifact path was reported.

Static site builds return their verified output directory and route HTML
files; use the directory for the complete asset tree. Web builds return the
verified `platforms/web` hosting directory and HTML/bootstrap/package files,
including JavaScript and WASM. Reports do not assert browser readiness or
publication. Site check/routes return planned paths, empty artifacts and a null
artifact directory, even if a previous build's files still exist. Routes have
`path`, `title`, `source`, `output`; public paths do not imply deployed URLs.

Direct finite site commands retain their existing ability to work without a
registered Static site target or target README. Generic `build --target
static-site` retains its target/scaffold validation. Invalid site configuration
now fails consistently in both modes rather than silently falling back to
default output settings.

Execution uses native filesystem paths in both modes, including OS-valid
non-UTF-8 paths. JSON serialization happens after execution; an unrepresentable
path produces a complete `report_unavailable` result with null project/state
fields, not partial JSON. A successful command may therefore have already
created files before a serialization failure. Use human output to inspect that
outcome. Consumers should tolerate additive fields; incompatible changes require
another schema version.

## Failures, diagnostics and recovery

Parsed command failures exit 1; successful commands exit 0. Malformed arguments
retain Clap's stderr diagnostics and exit 2 with empty stdout. Help/version
retain their normal output and exit 0. The flag changes serialization only.

`error` contains `code`, `message`, bounded `diagnostics`, and `recovery`.
Codes are `project_not_found`, `invalid_configuration`, `invalid_target`,
`target_not_configured`, `scaffold_missing`, `missing_toolchain`, `compile_failed`,
`site_failed`, `artifact_missing`, `report_unavailable`, `io_failed`, `interrupted`.
Recovery contains a known prerequisite when available and the original invocation
to retry after repair. Each invocation has `cwd`, `program` and separate `argv`
entries. Execute those entries directly through a process API; do not interpolate
or split a shell command. Source edits or tool installation remain prerequisites
when indicated; recovery is not executed automatically.

Finite subprocess output is drained concurrently, redacted and bounded on stderr
in both modes. At most 4 KiB per stream is retained, and failure diagnostics are
bounded to 8 KiB. Cargo metadata/compiler/report payloads are bounded to 16 MiB
and are not printed. Configuration errors omit parser source snippets. Credential
environment values, credential-bearing lines, authenticated URLs and compiler
source excerpts are redacted or omitted. Unknown secrets embedded in arbitrary
application logs cannot be inferred; applications should not emit them.

## Execution and compatibility boundaries

Setup uses the original project/scaffold routines. Generic build retains its
existing target dispatch, host checks, feature/variant validation, target
preparation and native-module calls. Content sites use shell report APIs.
Programmatic sites compile the selected/default binary, obtain its executable
from Cargo artifact messages, and run it with an ephemeral `--report-file`.
`build_from_cli` writes `SiteBuildReport` there. This shared path requires a
matching shell revision with report-file support in both modes; older custom
builders fail with `report_unavailable` rather than guess paths from prose.

The standalone `fission-command-process::diagnostic` utility has no dependency
on command reports, target types or renderers. Existing device arrays, package
manifests, readiness, release, publish and mutation JSON retain their protocols.
Doctor remains human-readable. Run/test/serve and preview lifecycle protocols
are outside this finite envelope. No serving refactor is included.
