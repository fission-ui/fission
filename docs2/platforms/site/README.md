# Static site target

This target writes the documentation site as HTML. The site shell renders
Markdown through Fission widgets; Docs2 supplies its own Compose pages and goal
tutorial tooling before that render begins.

- Add Markdown or MDX content under `content/`.
- Run `fission site routes --project-dir .` to list generated routes.
- Run `fission site build --project-dir .` from `docs2/` to write the configured `dist/site` output.
- Run `fission site serve --project-dir .` to build and serve the generated site locally.
- Run `fission package --target static-site --format static --release --project-dir .` to package the generated site.
- Unsupported interactive widgets fail during the static render instead of silently falling back to JavaScript.

Tutorial interaction belongs to a separately compiled Web app embedded in the
page. Rendering its surrounding static HTML does not exercise that app's
behaviour; test the preview and the tutorial's native target independently.
