# fission-shell-site

Static HTML shell for Fission applications and documentation sites.

`fission-shell-site` lowers real Fission widget trees and Markdown/MDX content routes into static HTML, CSS, and search metadata. It powers the Fission documentation site itself. Application developers normally use it through the `fission` facade with the `site` feature and through the `fission site` command:

```toml
[dependencies]
fission = { version = "0.2.0", features = ["site"] }
```

```sh
fission site serve --project-dir documentation
fission site build --project-dir documentation --release
```

## What it contains

- A static-site app model with custom widget routes and content routes.
- Markdown/MDX content loading with page metadata, sidebars, table-of-contents links, and templates.
- HTML generation from Fission nodes, plus generated CSS derived from widget styles and design-system values.
- Optional code highlighting, client-side search index generation, favicons, sitemap, robots output, canonical URLs, JSON-LD, and route-filtered page element injection.
- Link checking and production build validation used by `fission site check`.

## Design notes

This is a shell, not a separate website framework. Custom pages are written as normal Fission widgets. Content pages are rendered through Fission templates. The static shell visits the resulting Fission node tree and emits semantic HTML instead of opening an OS window or canvas.

## Documentation

See the static site guide at [fission.rs](https://fission.rs/docs/guides/static-sites/). The source for the live documentation site is in the repository `documentation` project.

## License

MIT
