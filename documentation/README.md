# Fission documentation website

This crate builds [fission.rs](https://fission.rs), including the product site,
developer documentation, API and crate reference pages, tutorials, blog, search
index and the embedded browser showcase. It is a real Fission static-site
application rather than a separate JavaScript documentation stack.

Most authored content lives below `content/`; reusable site components live in
`src/components/`; templates and route configuration are in `site/` and
`fission.toml`; public assets are staged below `static/`.

## Work locally

From the repository root, check routes and content before serving the site:

```bash
cargo run -p cargo-fission --bin fission -- site check \
  --project-dir documentation

cargo run -p cargo-fission --bin fission -- site serve \
  --project-dir documentation
```

The production workflow also builds the Web showcase and stages its generated
JavaScript and WASM under `documentation/static/showcase`. Those outputs are
generated deployment inputs and should not be hand-edited or committed.

## Content conventions

- Write user-facing prose in British English.
- Prefer complete, runnable examples over isolated fragments.
- Link to the exact reference page for unfamiliar APIs.
- Keep titles, descriptions, headings and navigation labels specific enough for
  developers to understand the result before following a link.
- Update navigation, footer links and the sitemap-visible route together when
  adding a first-class page.

The `/compose` page introduces the recommended interface authoring API. The
`/showcase` page embeds the static Web build used for browser qualification.

## Qualification

The website publishing workflow checks routes, the static dependency boundary,
generated search and metadata files, homepage semantics, showcase assets and
the rendered showcase layout. Run the corresponding local checks before using
CI as independent confirmation.

## Licence

Apache-2.0
