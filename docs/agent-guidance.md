# Offline application guidance

The installed CLI bundles one small generated application router, a local
target-neutral shared application reference and a `fission-web` skill with four
conditional browser references. Immediately after init, read the printed
instruction paths.
All app targets read `.fission/references/shared-app.md` for widget/state/identity,
reducer, build-scope handle and asynchronous-work rules. Website tasks load setup/targets,
widgets/state/routing, design/i18n or testing/review only when relevant.
No assistant connector, global skill installation or network request is needed.

```sh
fission skills check --project-dir ./my-app
fission skills check --project-dir ./my-app --json
fission skills update --project-dir ./my-app --json
```

Both operations are offline. Check is read-only. Update uses the assets compiled
into this CLI; it neither upgrades the framework dependency nor fetches newer
instructions. A missing asset can be created. An existing asset can be refreshed
only if its bytes match the manifest's recorded SHA-256 or a known exact bundled
template. A marker alone is never proof that a file is unmodified.

The bundle lives at the existing Git-root instruction location (the project
directory outside Git). Generated entrypoints are `AGENTS.md`, or
`AGENTS.fission.md` when the former contains user/contributor instructions.
When both are customized, init reports a managed `.fission/AGENTS.md` fallback.
All custom root and nested instructions remain intact; read them alongside the
reported fallback. Init does not append links to customized instructions.
Project paths become absolute by joining the current directory; links and caller
spelling are preserved. Git discovery walks lexical ancestors, including a mapped
or symlinked caller namespace. Applicable instruction paths include intermediate
ancestors down to the app. Managed destination components are still checked for
symlinks/collisions; explicitly supplying a linked project root is supported.
The skill is `.fission/skills/fission-web/SKILL.md`; its relative references
resolve entirely inside the generated tree.

`.fission/guidance-manifest.json` schema 1 records guidance version 3, CLI asset
version, framework API version, and relative managed paths with exact SHA-256s.
The included maintained Markdown assets and a single Rust asset table generate
the bundled manifest/hashes, so no second hand-maintained copy can drift.
The manifest itself is metadata and is excluded from its own content hash.
Root repository contributor guidance is independent of generated app assets.

Command-core installation is silent and returns `GuidanceInstallation` facts:
the full guidance result, actual instruction/fallback paths, and optional paths
to existing shared/Web assets. The CLI renders those facts. Init's authority
returns them after scaffolding completes; a common CLI result integration should
carry the returned paths rather than rediscovering only root instruction names.

The CLI package version describes the installed assets. The app dependency is
reported separately from `Cargo.toml` and available local `Cargo.lock`.
Only an unambiguous crates.io lockfile version matching the bundle API version
and the declared requirement gives `resolved_version_match` evidence.
This still does not prove an app compiles or behaves correctly. A missing lock,
workspace inheritance, patches, alternate sources, malformed versions and
local-path/Git dependencies are unresolved/unverified, with precise reasons.
A local crate's declared version is reported when readable but never proves its
checkout matches the bundle. There is no network update check or
`updateAvailable` field.

JSON results use a narrowly scoped guidance schema, not a general command-result
contract. They include operation/status, bundled and installed metadata,
framework compatibility, per-file health (current/missing/stale/customized/unsafe),
coverage limits, absolute instruction paths, findings and recovery argument
arrays. Pass recovery argv directly to a process API rather than shell-joining it.

Exit codes:

| Operation | Code |
| --- | --- |
| Check: assets current and resolved matching framework version | 0 |
| Check: missing/stale/customized assets, unknown/mismatched dependency or error | 1 |
| Update: safe asset installation, even if dependency compatibility is unresolved | 0 |
| Update: conflict/validation/write failure | 1 |
| Malformed arguments | 2 (Clap diagnostics on stderr) |
| Help/version | 0 |

For malformed arguments in the recognized `skills` command containing an exact
`--json` token, stdout also contains one parseable
`invalid_arguments` guidance result. Unknown top-level commands retain ordinary
Clap behavior. Help is human output, including when `--json` is present.

Conflicts preserve bytes and block all managed-file writes. Back up the indicated
file, explicitly merge its instructions, or move it aside after preserving user
content, then rerun check/update. Do not delete a customized instruction merely
to clear a conflict. Unmodified legacy v1 instructions migrate only when they
match a bundled v1/v2 historical entrypoint template byte-for-byte; unknown/edited legacy
markers remain conflicts. Unrelated local skills are never claimed or removed.
The manifest cannot claim arbitrary paths: only bundled asset paths and one
known instruction entrypoint are accepted. Traversal, duplicate paths,
file/directory collisions and symlink destinations are rejected before writes.

Updates stage changed files under `.fission/guidance-transaction/`, storing
original bytes and `journal.json`. Existing destinations move to `displaced/`
before replacements are created with create-new semantics; no rename must replace
an existing file on Windows, and concurrent destinations are not overwritten.
Files are written individually and the manifest is committed last. Ordinary
commit errors roll back unchanged replacements. A changed/incomplete replacement
keeps recovery data instead of erasing another writer's bytes. Interruption
leaves the journal/originals available and blocks further updates. Inspect each
journal entry (its array index names the backup), including `displaced/`, while
preserving concurrent edits: restore present originals;
remove only managed destinations explicitly marked originally absent. Then
remove the transaction directory after checking the recovery. Do not run
concurrent instruction editors/updaters; this is a recoverable local update,
not a filesystem-wide atomic transaction or a hostile-writer security boundary.

This bundle is based on main's existing static-site/Web commands. Serving uses
`fission run --target web` and `fission site serve`; testing/review capabilities
must be checked under the installed `fission test` interface. No standalone
preview/review interface is taught. Browser smoke readiness and semantic geometry
coverage are partial; verified in-session resize and generic test-control frame
acknowledgment are not available on this base. The guidance JSON schema is a domain
payload and does not prescribe a generic command execution/result architecture.

See [validation evidence and limits](testing/agent-guidance.md) for package checks,
fresh generated-app examples and the observed Web capture limitation.
