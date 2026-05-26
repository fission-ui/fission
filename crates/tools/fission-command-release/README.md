# fission-command-release

Release metadata, signing, beta, review, and authorization workflows for the `fission` command.

`fission-command-release` contains the release-lifecycle commands that sit around packaging and distribution. It is an implementation crate for the public `fission` executable.

## What it contains

- Release metadata editing and validation helpers.
- Signing and credential setup command flows.
- Store/provider release preparation, review, beta-track, rollout, and submission workflows.
- Integration points for secure credential storage through `fission-credentials`.

## Design notes

`fission.toml` remains the authoritative project manifest, but long-form release notes, screenshots, localized store text, and provider-specific files should live in referenced files so the manifest stays readable.

## Documentation

See [Release and distribute](https://fission.rs/docs/release-and-distribute/overview/) and the post-build lifecycle RFC in the repository `docs` directory.

## License

MIT
