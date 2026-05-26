# fission-command-site

Static-site command implementation for the `fission` command.

`fission-command-site` wires the `fission site` CLI surface to `fission-shell-site`. It is part of the single installed `fission` executable.

## What it contains

- `fission site check` for route, content, link, metadata, and production-readiness validation.
- `fission site build` for generating static HTML, CSS, search data, favicons, sitemap, robots output, and page metadata.
- `fission site serve` for local development preview.

## Documentation

See [Static sites](https://fission.rs/docs/guides/static-sites/) and the CLI reference at [fission.rs](https://fission.rs/docs/reference/cli/overview/).

## License

MIT
