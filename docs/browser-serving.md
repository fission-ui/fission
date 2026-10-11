# Owned browser serving

The existing commands build and serve their selected output:

```sh
fission run --target web --project-dir ./app --port 0 --no-open
fission site serve --project-dir ./app --port 0 --no-open
fission run --target web --project-dir ./app --port 0 --mount /repository-name/ --no-open
```

Port zero binds an OS-selected port and reports its actual URL. Web run retains
its existing available-port search for occupied fixed ports. Site serve fails
on an occupied fixed port. Neither command adopts or terminates another listener.
Mounted Web deep routes fall back to the entry document; missing asset files
return 404. The build configuration and generated app files are preserved.

The build and local readiness sequence has a default 300-second deadline;
`--startup-timeout-seconds 1..3600` changes it. Build commands execute through
their existing authority in an owned process group/Job Object with bounded
diagnostic tails. There are no hidden build/probe workers. Programmatic static
entries run their existing `build` command, then serve the configured output
through the same owned file server as content sites.

`local_assets` readiness verifies the expected HTML, redirects, required local
scripts/bootstrap/WASM, stylesheets and their literal asset references against
the built files. Optional favicons and renderer diagnostic POSTs are not startup
requirements. External resources and dynamic references are outside this local
check. Browser execution, renderer readiness and LiveTest remain under testing:

```sh
fission test --target web --project-dir ./app
fission test --target static-site --project-dir ./app
```

Only testing enables Web development test control. Ordinary run/serve does not.

## Wrappers and shutdown

```sh
fission site serve --project-dir ./app --port 0 --no-open --json --stdin-control
```

Human and JSON output use the same execution path. JSON stdout is a flushed
line stream of `fission.cli-event.v1` events; diagnostics go to stderr. This
event envelope is distinct from finite `fission.cli-result.v1` outcomes.

```json
{"schema":"fission.cli-event.v1","action":"site serve","project_dir":"./app","owner_pid":123,"event":{"phase":"ready","url":"http://127.0.0.1:49152/","readiness":"local_assets","verified_local_assets":4}}
```

Phases are `building`, `ready`, `stopped`, or `failed`. Failed events include a
message, a resource-release fact, and the original retry argv. Correct the
reported prerequisite before retrying. `ready` claims local assets, not browser
rendering. `--json` on run currently requires an explicit Web or Static site
target and attached execution; other run targets retain their human behavior.
Malformed CLI arguments retain Clap's normal diagnostic and nonzero exit.

Retain the attached process and stdin. With `--stdin-control`, send `stop` plus
a newline or close stdin; Ctrl+C/SIGTERM also stops startup or serving. Wait for
the terminal event and process exit. Cleanup affects only owned build trees and
the listener; never kill processes by port or trust a saved PID. An idle/slow
request has a total deadline, so it cannot indefinitely delay owned shutdown.

Legacy detached Web run remains available with a fixed port and root mount.
Use attached serving for zero-port URLs, mounted serving, JSON events, and stdin
control. No public preview command or HTTP lifecycle/control service is added.

## Internal reuse

`fission_command_site::serving::OwnedServer` owns the actual listener and file
request thread. `ServingSession::start` verifies built assets before returning.
`fission_command_process::StartupContext` applies one deadline/cancellation
policy to existing `run_status` calls without a second build dispatcher.
`fission_command_run::serving::build_and_serve` combines existing validated build
dispatch with the shared server for browser and visual testing; callers own the
returned session and their `LiveTestClient`. Drop and explicit stop release
those owned resources. Browser frame and screenshot correctness are defined by
the generic test-driver/test-control contract, not a serving protocol.
