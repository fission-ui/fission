# Browser preview lifecycle

`fission preview` builds and validates an attached local preview for Static site
and client Web/WASM targets. Existing `run`, `build`, `site serve`, and `serve-web`
commands remain available. Use the preview lifecycle when an agent or CLI wrapper
needs readiness, an exact URL, bounded startup, and owned shutdown.

After `fission init`, read the generated `AGENTS.md` and any referenced
`AGENTS.fission.md` immediately. Retain `fission.toml` and use `fission add-target`
to configure supported targets. This interface works with the existing content
site and Web scaffolds; it does not require a different website starter.

```sh
fission add-target static-site web --project-dir ./app
fission preview --target static-site --project-dir ./app --port 0
fission preview --target web --project-dir ./app --port 0 --mount /repository-name/
fission preview --target web --project-dir ./app --port 0 --live-test
```

The default host is `127.0.0.1`, port is `8123`, mount is `/`, and entry is
`index.html`. Port `0` asks the OS for an available port. An occupied fixed port
is an explicit failure; Fission neither switches ports silently nor terminates
the occupying process. The reported URL uses the actual bound address and port.
Static output comes from the existing `[site].out_dir` configuration; Web output
comes from `platforms/web`. Mounting does not rewrite configuration or assets.
Use relative asset URLs when the same build must work under a repository mount.
Web deep-route fallback HTML receives a mount-root `base` element when it has
no explicit base, so nested refreshes load the same relative bootstrap files.
Set the application's existing `WebNavigationConfig::base_path` to the mount
when it uses pathname routing; the server does not rewrite logical app routes.
`--entry content/getting-started/index.html` selects an existing content route if
the configured site has no root entry. Missing entries fail instead of inventing
a successful root URL.

Startup has one deadline across building, serving, asset verification, and the
optional browser probe. `--startup-timeout-seconds` defaults to 300 and accepts
1–3600. Build/probe output retains only the last 32 KiB from each stream. Missing
tools, compile errors, invalid mounts/entries, bind errors, missing/invalid assets,
unexpected server exits, cancellation, and timeouts are reported explicitly.

Required GET responses must match the expected build files. HTML scripts,
stylesheets, images, meta-refresh destinations, literal JS imports/URL/fetch
references, and CSS resource references are checked; Web
also requires a bootstrap chain reaching a valid WASM file. A bound socket or
HTTP 200 from an unrelated server/SPA fallback is insufficient. Favicons and the
renderer diagnostic POST are best effort and do not gate readiness. External
assets and arbitrary dynamically constructed application fetches are outside
this local asset check; verify those through application/browser tests.

Default readiness is `local_assets`: the build is locally usable, but no browser
render is asserted. `--live-test` is Web-only development mode. It compiles the
existing test bridge, then uses the existing Rust browser driver in a temporary
Chrome session to verify a renderer, presented content frame, bridge, pump,
and tree response. It reports `live_test_ready: true` only after that probe
succeeds. It does not expose an HTTP
control service or attach to the user's browser tab. Rebuild Web without
`--live-test` before production packaging. The static output remains ordinary
files without a control-service dependency.

## Wrapper contract

```sh
fission preview --target web --project-dir ./app --port 0 \
  --mount /repository-name/ --json --stdin-control
```

Stdout is a flushed JSON event stream with schema `fission.preview.v1`. Stages
are `validating`, `building`, `serving`, `verifying`, optionally
`verifying_live_test`, `ready`, and finally `stopped` or `failed`. Each event
contains a unique `session`, the attached CLI's `owner_pid`, target, mount,
actual URL when known, readiness level, and stage-specific `detail`. The server
is owned by that CLI process, not a separate detached daemon. `failed` includes
`failed_stage`, `message` with the bounded diagnostic tail, `retry_argv`, and
recovery guidance. It exits nonzero. Ordinary human usage remains readable;
diagnostics stay on stderr in JSON mode.
Terminal events include `owned_resources_released`. If worker-tree cleanup
fails at the OS level, it is `false` and the error is reported even during
cancellation. Inspect that session's owned process tree before retrying.

Retain the spawned CLI handle and its stdin. With `--stdin-control`, write
`stop\n` or close stdin to cancel at any stage. Without that flag, stdin EOF is
ignored. Ctrl+C/SIGINT and SIGTERM (Unix), or Ctrl+C (Windows), also stop the
attached preview. A stopped/cancelled session releases its owned listener and
build/probe process tree and exits successfully. Wait for its terminal event
and exit before retrying. Do not kill by port or issue a stop command against an
unverified persisted PID. No session database or reusable remote-control token
is needed.

The supervisor uses a fresh Unix process group or Windows Job Object. Internal
build workers keep nested Cargo/wasm-pack commands in that owned tree. This
covers supported cancellation and termination signals; an uncatchable OS kill
or a tool deliberately escaping its process group is outside the attached
shutdown contract. The listener itself closes when the CLI process exits.

## Failure recovery

- Compile/tool failure: correct the compiler error or install the missing target
  tool (`fission doctor web --strict`), then repeat the reported arguments.
- Occupied port: retry with `--port 0` or choose another port. The unrelated
  listener survives.
- Mount/entry/asset failure: check the generated config's output directory,
  `--entry`, and mount-relative references. Required asset failures cannot be
  dismissed as renderer diagnostics or favicons.
- Timeout: inspect the captured tail, resolve a hung tool, or increase the
  startup deadline within its supported bound. The owned worker is terminated.
- Browser probe failure: inspect the driver diagnostic, Chrome availability,
  renderer, and opt-in bridge. A failed probe never emits a ready event.

This command does not provision SSR, backends, hosting, deployment, native apps,
or an MCP service.
