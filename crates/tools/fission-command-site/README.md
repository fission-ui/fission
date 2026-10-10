# fission-command-site

Static-site command implementation for the `fission` command.

`fission-command-site` wires the `fission site` CLI surface to `fission-shell-site`. It is part of the single installed `fission` executable.

## What it contains

- `fission site check` for route, content, link, metadata, and production-readiness validation.
- `fission site build` for generating static HTML, CSS, search data, favicons, sitemap, robots output, and page metadata.
- `fission site serve` for local development preview.

## Local serving paths

The shared server used by `run --target web`, `site serve`, and browser tests
keeps the selected root lexical and works with mapped/WebDAV paths without
filesystem canonicalization. URL paths allow ordinary relative components;
existing descendant symlinks and Windows reparse points are rejected. The
explicitly selected root may itself be reached through a link or mount. Keep
build output under your control while serving; metadata checks are not a
sandbox against concurrent filesystem replacement.

## Documentation

See [Static sites](https://fission.rs/docs/guides/static-sites/) and the CLI reference at [fission.rs](https://fission.rs/reference/cli/overview/).

## License

Apache-2.0
