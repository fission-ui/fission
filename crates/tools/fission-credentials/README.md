# fission-credentials

Credential vault helpers for Fission tooling.

`fission-credentials` stores provider credentials and tokens for the `fission` command without writing raw secrets into project files. It is an implementation crate used by release, package, and distribution commands.

## What it contains

- OS keyring integration for storing or protecting encryption material.
- Encrypted local credential records for providers that require tokens, service account keys, signing material, or upload credentials.
- Small APIs used by `fission auth`, package, release, and distribute workflows.

## Design notes

Project configuration should reference credential names or provider profiles, not embed secrets. CI environments can provide credentials through environment variables or explicit setup commands where appropriate.

## Documentation

See [Release and distribute](https://fission.rs/docs/release-and-distribute/overview/) and the post-build lifecycle RFC in `docs/post-build-lifecycle.md`.

## License

MIT
