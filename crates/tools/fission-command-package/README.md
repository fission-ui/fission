# fission-command-package

Packaging, readiness, and distribution workflows for the `fission` command.

`fission-command-package` implements the package and distribution parts of the developer lifecycle. It is composed into the single installed `fission` executable by the `cargo-fission` crate.

## What it contains

- `fission readiness package` checks for project metadata, platform support files, signing inputs, and target prerequisites.
- `fission package` artifact generation for supported platform/package combinations such as static sites, Linux `.run`, macOS app/pkg outputs, Windows/MSIX flows, Android packages, and iOS archive handoff paths.
- `fission release-content` validation for release metadata, screenshots, changelogs, store text, and localized release assets.
- `fission distribute` provider integrations for static hosting, GitHub Releases, S3-compatible storage, and store/provider handoff flows.

## Design notes

Fission uses platform-provider tools where the platform requires them. This crate orchestrates those tools, validates inputs, records generated artifacts, and keeps credentials out of project files through the shared credential store.

## Documentation

See [Build and package](https://fission.rs/docs/build-and-package/overview/) and [Release and distribute](https://fission.rs/docs/release-and-distribute/overview/).

## License

MIT
