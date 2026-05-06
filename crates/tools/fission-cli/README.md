# fission-cli

`fission-cli` provides the first-party Fission project scaffolding commands:

- `fission init`
- `fission add-target`
- `cargo fission ...`

## Usage

Create a new app:

```sh
fission init my-app
```

Create a new app against a local Fission checkout:

```sh
fission init my-app --local-path /path/to/fission
```

Add more platform targets:

```sh
cargo fission add-target web ios android --project-dir my-app
```

## Current platform status

- `windows`, `macos`, `linux`: scaffolded and runnable through the generated desktop entrypoint
- `ios`, `android`: scaffolded by the CLI and verified to cross-compile after `add-target`, but generated projects do not yet get native packaging or launcher files
- `web`: scaffolded only; `fission-shell-web` is still pending

The CLI writes platform state to `fission.toml` and creates `platforms/<target>/README.md` notes with the current prerequisites and next steps for each target.

See also:

- `../../../docs/cli-and-targets.md`
- `../../../docs/platform-smoke-tests.md`
