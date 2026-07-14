# Static site target

This project uses the Fission static site target only.

The site is a real Fission app: custom pages are Rust widgets, Markdown and MDX files under `content/` become content routes, sidebars live under `site/`, and static assets live under `static/`.

Useful commands:

- `fission site routes --project-dir documentation` -- list generated custom and content routes
- `fission site check --project-dir documentation` -- render all routes and validate internal links
- `fission site build --project-dir documentation` -- write the configured output directory
- `fission site serve --project-dir documentation` -- build and serve locally on `127.0.0.1:8123`

The generated output under `dist/site` is build output and should not be committed.
