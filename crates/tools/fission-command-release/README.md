# fission-command-release

Release metadata, signing, beta, review, and provider-readiness workflows for the `fission` command.

`fission-command-release` contains the release-lifecycle commands that sit around packaging and distribution. It is an implementation crate for the public `fission` executable.

## What it contains

- Release metadata editing and validation helpers.
- Signing and provider credential-readiness command flows.
- Store/provider release preparation, review, beta-track, rollout, and submission workflows.

## Design notes

`fission.toml` remains the authoritative project manifest, but long-form release notes, screenshots, localized store text, and provider-specific files should live in referenced files so the manifest stays readable.

The crate does not provide an application-facing library API. Commands are
assembled by `cargo-fission`, reuse package metadata produced by the packaging
crates, and keep provider-specific validation at the release boundary. Release
workflows must fail clearly when required credentials, signatures or review
metadata are unavailable; they must not silently publish a partial release.

Tests use provider fixtures and temporary project manifests. They do not require
real store credentials and must never print secrets supplied by a developer's
environment.

## Documentation

See [Release and distribute](https://fission.rs/docs/release-and-distribute/overview/) and the post-build lifecycle RFC in the repository `docs` directory.

## License

Apache-2.0
