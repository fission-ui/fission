# fission-command-core

Shared project model for the `fission` command.

`fission-command-core` contains the common manifest, target, capability, and platform configuration types used by the command modules. It is an implementation crate for the installed `fission` developer tool, not a crate most application projects should depend on directly.

## What it contains

- `fission.toml` loading, saving, and idempotent project initialization helpers.
- Target and platform capability models shared by run, package, release, and site commands.
- Generated support-file helpers for platform directories.
- Common CLI argument types reused by command crates.

## Developer workflow

Install the public command with:

```sh
cargo install cargo-fission
```

That exposes the single `fission` executable. This crate is part of the internal command implementation behind that executable.

## Documentation

See [fission.rs](https://fission.rs/docs/reference/cli/overview/) for the public CLI reference.

## License

MIT
