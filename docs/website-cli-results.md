# Finite website CLI results

An agent can request one final `fission.website-result.v1` JSON object on
stdout. Human output remains the default. This protocol covers these calls:

```sh
fission init "my website" --name my-website --json
# Read every data.instructions path immediately, before editing the app.
fission add-target static-site --project-dir "my website" --json
fission site routes --project-dir "my website" --json
fission site check --project-dir "my website" --json
fission site build --project-dir "my website" --json
fission build --target static-site --project-dir "my website" --json
# Or add and build Web/WASM:
fission add-target web --project-dir "my website" --json
fission build --target web --project-dir "my website" --json
```

Init supports its existing name/app-id/local-path options and retains its
existing desktop defaults. Add-target supports one or both website targets in
JSON mode. Build requires explicit `--target web` or `--target static-site` in
JSON mode; its existing `--release` option applies. Web also supports the
existing `--features` and `--no-default-features` options. Site build/check
support `--release`; site routes does not. The `site` target alias is accepted
and serialized as `static-site`. No `wasm` target name is introduced: Web uses
wasm-pack. No website starter/scaffold from unmerged PR #255 is included.

The public Rust types are in `fission_command_core::website`. `CommandResult`
contains `schema`, `action`, absolute `project_dir`, `selected_targets`, optional
observed project `state`, and a tagged `outcome`. State contains the application
name from fission.toml, configuration path and actual configured targets, including the existing
init defaults. Actions are `init`, `add_target`, `build`, `site_build`,
`site_check`, `site_routes`. Enum spellings and required fields are part of v1.
`project_dir` is null if it cannot be resolved or represented in JSON.
Structured mode requires UTF-8 project and working-directory paths. A
non-UTF-8 project argument emits `invalid_configuration` without creating the
project. Reports are serialized before writing stdout; an unrepresentable
discovered path emits `report_unavailable` instead of a partial JSON object.
Consumers should tolerate additional fields; incompatible changes require a
new schema version. `Outcome` is `success` with `data` or `failure` with `error`.

For a content-site build, representative output is:

```json
{"schema":"fission.website-result.v1","action":"site_build","project_dir":"/work/my website","selected_targets":["static-site"],"state":{"name":"my-website","config_path":"/work/my website/fission.toml","configured_targets":["linux","macos","static-site","windows"]},"outcome":"success","data":{"artifact_dir":"/work/my website/target/fission/site","planned_output_dir":"/work/my website/target/fission/site","artifacts":["/work/my website/target/fission/site/content/index.html"],"routes":[{"path":"/content/","title":"Home","source":"/work/my website/content/index.md","output":"/work/my website/target/fission/site/content/index.html"}],"instructions":[],"next_steps":[{"instruction":"Validate all routes before consuming the artifact directory.","invocation":{"cwd":"/work","program":"fission","argv":["site","check","--json","--project-dir","/work/my website"]}}]}}
```

Actual title, routes and paths come from the shell report. `artifact_dir` is
verified to exist only after a successful build. Static-site artifacts list
reported route HTML files, not an exhaustive asset inventory; use the whole
artifact directory when packaging for GitHub Pages or another static host.
Web returns `platforms/web` as the hosting root, including its HTML/bootstrap
and actual files from `platforms/web/pkg`. The result verifies expected HTML,
JavaScript and WASM files exist; it does not claim browser readiness or a
successful deployment. Check/routes return `planned_output_dir` and planned
route output paths, with null `artifact_dir` and empty `artifacts`, even when
files from an earlier build already exist. Routes are public paths, not preview
or deployment URLs.

Keep generated `fission.toml` and use add-target for target configuration. Read
generated `AGENTS.md` and any referenced/sibling `AGENTS.fission.md` immediately
after init; `data.instructions` reports the actual guidance paths, including a
Git root above a nested app. Custom guidance is preserved by the existing init
policy.

## Failures and recovery

Parsed command failures emit one result and exit nonzero (currently 1).
Successful operations exit 0. Stdout contains only the result. Subprocess
stdout/stderr go through concurrent drains, with at most 4 KiB retained per
stream, forwarded to stderr on completion; `error.diagnostics` is bounded to
8 KiB combined. Structured site compiler/metadata/report payloads have a
16 MiB bound and are never printed on success. Configuration errors avoid
echoing configuration source text, and compiler source excerpts are omitted.
Diagnostics redact inherited/explicit
environment values whose names indicate credentials (tokens, passwords,
secrets, authentication and private/access/API keys). No environment dump or
command debug dump is printed. Arbitrary secrets embedded in application
source/logs with no credential environment value are not automatically known
to the CLI; applications should not emit them.

Malformed arguments (unknown flags, missing required arguments or invalid
enum spellings) are rejected by Clap: stderr diagnostics, empty stdout, exit
2. They do **not** emit this schema. `--help`/`--version` retain normal Clap
output and exit 0. JSON mode is command-local, not a global flag. Other
commands have not implicitly adopted this contract.

| Stable error.code | Meaning / prerequisite |
| --- | --- |
| `project_not_found` | No fission.toml; register with the supplied init invocation and read guidance. |
| `invalid_configuration` | Repair fission.toml, Cargo.toml, options or referenced paths; no source contents are echoed. |
| `invalid_target` | Select an explicit supported website target; configured alternatives are returned where available. |
| `target_not_configured` | Follow add-target recovery, then retry. |
| `scaffold_missing` | Follow add-target to repair missing scaffold files, then retry. |
| `missing_toolchain` | Required executable or the selected compiler's wasm32 standard library is unavailable; install it and run the supplied doctor invocation. |
| `compile_failed` | Cargo/wasm-pack compilation failed; inspect diagnostics, repair source/dependencies, retry. |
| `site_failed` | Route discovery, rendering, link validation or the compiled site application failed. |
| `artifact_missing` | Successful subprocess did not produce required reported files. |
| `report_unavailable` | Typed builder report or Cargo artifact payload is unavailable, invalid or too large. |
| `io_failed` | File/process/diagnostic I/O could not complete. |
| `interrupted` | A supervised subprocess was interrupted. |

Recovery steps contain instructions (including edit/install prerequisites) and
an invocation: `cwd`, `program`, `argv`. Pass argv entries directly to a process
API, preserving spaces; do not split or shell-interpolate them. The final step
retries the requested operation with its parsed options. Nothing is executed
automatically. Recovery reflects supported commands and actual configured
targets; it cannot repair arbitrary Rust code automatically.

A configuration failure can be consumed as:

```json
{"code":"invalid_configuration","message":"Cannot load [site] configuration or its referenced files; repair fission.toml and the referenced paths.","diagnostics":"","recovery":[{"instruction":"Repair the configuration first, then retry.","invocation":{"cwd":"/work","program":"fission","argv":["site","check","--json","--project-dir","/work/my website"]}}]}
```

For `compile_failed`, diagnostics include a bounded compiler excerpt such as
`error[E0308]: mismatched types`. Repair the indicated Rust source/Cargo
dependency, then execute the provided retry argv. Restoring a valid TOML value
or correcting the Rust type error is required before retry can succeed.

## Library and compatibility boundaries

Init/add-target reuse command-core operations. Website builds reuse Web target
preparation and wasm-pack argument wiring. Content sites return existing shell
reports. Programmatic `[site].entry` sites compile a selected/default binary,
get its executable path from Cargo artifact messages, then execute it with an
ephemeral `--report-file`. `build_from_cli` writes the typed `SiteBuildReport`
there; app stdout cannot contaminate the CLI result. This requires a matching
Fission shell revision with report-file support. Older/custom builders that do
not use that shell can still use human mode; structured mode returns failure
and recovery rather than guessing paths from human output. The report file is
removed after use. Compilation and builder execution failures have distinct
codes. No preview lifecycle state machine or hosting code is changed.

Existing package artifact manifests, readiness reports, release lifecycle and
mutation JSON, publish `schema_version: 1`, and device arrays retain their
shapes. Doctor on this base has human output and `--strict`, no JSON mode.
Runtime `DiagEvent` records are a separate diagnostics protocol. The unmerged
preview PR #259's `fission.preview.v1` lifecycle events describe an attached
process, whereas this protocol describes a finite result; neither is adapted
or wrapped here. This feature does not extend JSON coverage to run/test/serve,
SSR, native/mobile/terminal targets, deployment or provider authentication.
