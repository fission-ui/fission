# RFC: Static Site Renderer Target

Status: proposal  
Audience: Fission compiler, renderer, shell, website, documentation, and release tooling implementers  
Scope: generating production static websites from Fission applications

## 1. Summary

Fission should support a static site target that renders Fission routes and Markdown content collections into real HTML documents, CSS assets, optional JavaScript enhancement modules, sitemaps, metadata, and static assets. The output must be a multi-page static website, not a client-rendered single-page application.

The primary rule is that page content must exist in the generated HTML before JavaScript runs. JavaScript may enhance behavior where needed, but it must not be required for reading, crawling, linking, or rendering the main content of a route. This preserves direct URLs, search indexing, link previews, accessibility, and long-term archiveability.

The target has two primary authoring modes:

1. custom pages, declared by the site build harness as route entries that render normal Fission widgets through the existing Fission router/widget model; and
2. content pages, discovered from configured Markdown folders such as `content/`, with front matter used to select metadata and the page template.

The static target is a shell/renderer target. It must not introduce an HTML-first application model, a replacement router, or template APIs that bypass `Widget::build`.

The target should use small progressive-enhancement controllers for simple interactive islands, and WASM-backed islands when a route needs Rust action/reducer logic in the browser. Production builds should be minified locally with Rust tooling where possible and must not require a hosted build service.

## 2. Goals

- Generate one HTML document per custom route and generated content route.
- Generate content routes from configured Markdown folders without requiring one Rust route per document.
- Generate semantic HTML, CSS, and minimal JavaScript enhancement files.
- Keep more than 90% of a typical documentation/marketing site static by default.
- Preserve search-friendly page content, canonical URLs, metadata, sitemap, and robots output.
- Support interactive islands only where declared or required by widgets.
- Avoid a client router by default.
- Use Fission routing, design-system tokens, and widgets as the authoring model.
- Provide a default documentation template for Markdown pages and allow per-page template overrides from front matter.
- Make unsupported static rendering fail at build time for custom routes and generated content routes.
- Avoid expensive external services and SaaS dependencies.
- Minify production output with local Rust tooling.

## 3. Non-goals

- Turning every Fission web app into a static website automatically.
- Requiring every widget to support static rendering.
- Requiring a server for basic site output.
- Shipping all app runtime code to the browser by default.
- Using JavaScript to generate primary page content after load.
- Building a general-purpose hosted CMS.
- Hiding unsupported widgets behind empty placeholder markup.

## 4. Command model

```text
fission add-target site --project-dir .
fission site build --project-dir .
fission site build --project-dir . --release
fission site serve --project-dir .
fission site check --project-dir .
fission site check --project-dir . --seo --links --a11y --json
fission site routes --project-dir .
```

`fission build --target static-site` may be an alias for `fission site build`, but the `site` subcommand is useful because static sites need route, metadata, link, SEO, content, and browser-code checks that are not normal app-package concerns.

### 4.1 Build pipeline

`fission site build` runs a build-time site renderer, not a browser crawler.

Required pipeline:

1. read `fission.toml`;
2. compile the project for the site build target with the static-site feature set enabled;
3. build or run a generated site harness that links the user's crate and calls the configured site entry point;
4. enumerate custom routes from the harness route manifest while using the existing Fission router/widget types to render each route;
5. scan configured Markdown content collections from `site.routes`;
6. parse front matter and create content routes;
7. resolve templates, metadata, canonical URLs, assets, design-system tokens, and data loaders;
8. render each route by calling `Widget::build`, lowering the resulting `Node` tree to Core IR plus Semantics;
9. visit the route Core IR and lower it to semantic HTML and extracted CSS;
10. emit declared DOM controllers and WASM islands only for routes that require them;
11. generate search indexes, sitemap, robots, route manifest, and asset manifest;
12. run validation checks;
13. minify and precompress in release mode when configured;
14. write the artifact manifest for post-build distribution.

The generated harness exists because the CLI cannot instantiate arbitrary Rust widgets by name at runtime. It must compile and link the user's site crate, then call a known entry point selected by convention or project configuration. A configured `entry` path is resolved while generating Rust harness source; it is not runtime reflection. The harness is a build tool; it is not shipped to the browser.

## 5. Configuration

Static site configuration belongs in `fission.toml`.

```toml
[site]
entry = "crate::site_app"
base_url = "https://fission.example.com"
out_dir = "dist/site"
default_locale = "en-US"
pretty_urls = true
minify = true
generate_sitemap = true
generate_robots = true
canonical_host = "https://fission.example.com"
progressive_enhancement = true
navigation_enhancement = "optional"

[site.assets]
public_dir = "site/public"
fingerprint = true
inline_critical_css = true

[site.seo]
default_title = "Fission"
default_description = "Build production apps and static sites with Fission."
default_image = "site/public/social-card.png"

[[site.routes]]
kind = "content"
path = "/content"
source = "content"
template = "fission_shell_site::documentation"
```

`site.routes` is for configured generated route sources such as Markdown content collections. The default content route maps Markdown files under `content/` to URLs under `/content` and renders them with the documentation template supplied by the static site shell.

The shorthand form below is equivalent to `kind = "content"`, `source = "content"`, and `path = "/content"`:

```toml
[[site.routes]]
path = "/content"
template = "fission_shell_site::documentation"
```

Custom designed pages do not live in `fission.toml`. They are declared in Rust as normal Fission widgets and routed with the existing router model. The site harness enumerates the route paths and asks the application to render each path; it does not use a separate site router:

```rust
use fission::prelude::*;
use fission::widgets::{Route, Router};

pub fn site_routes() -> &'static [&'static str] {
    &["/", "/showcase/", "/pricing/"]
}

pub fn site_root(current_path: String) -> impl Widget<SiteState> {
    Router {
        current_path,
        routes: vec![
            Route::new("/", HomePage),
            Route::new("/showcase/", ShowcasePage),
            Route::new("/pricing/", PricingPage),
        ],
        not_found: None,
    }
}
```

The exact helper constructors can evolve, but the invariant cannot: custom routes render through the existing Fission router/widget path, not through an HTML-first site API.

The static route graph is the union of:

1. custom routes declared by the site harness route manifest; and
2. content routes generated from configured `site.routes` content collections.

The static site builder must not discover the authoritative route graph by launching a browser and crawling links. Link crawling is a validation step, not a route definition mechanism. Markdown discovery is allowed only inside folders explicitly configured in `fission.toml`.

## 6. Route and content model

The static target needs an enumerable route model.

```rust
trait StaticSite {
    fn custom_routes(&self) -> Vec<StaticRoute>;
    fn render_custom_route(&self, route: &StaticRoute, ctx: &StaticBuildCtx) -> View;
}

struct StaticRoute {
    path: String,
    locale: Locale,
    metadata: RouteMetadata,
    source: StaticRouteSource,
}

enum StaticRouteSource {
    Custom,
    MarkdownContent(MarkdownContentRoute),
}

struct MarkdownContentRoute {
    source_path: PathBuf,
    collection_path: String,
    template: TemplateId,
    front_matter: FrontMatter,
    body: String,
}
```

Rules:

- every output page must correspond to either a custom router route or a configured content route;
- every custom route must be declared explicitly in the site harness route manifest and rendered through the existing Fission router/widget model;
- every Markdown content route must come from a configured `site.routes` source folder;
- every generated route must render deterministically at build time;
- route metadata must be available before rendering the HTML head;
- dynamic route generation must be data-driven and finite;
- route generation must be repeatable in CI;
- routes must not depend on browser-only APIs.

Dynamic content is allowed if it is resolved at build time through declared data loaders. If the content cannot be resolved during the static build, the route is not static and must fail or be moved to a server-backed/web-app target.

### 6.1 Markdown content collections

A content route entry scans a configured folder for Markdown files. The default folder is `content/`; the default URL prefix is `/content`; the default template is the documentation template supplied by the static site shell.

Example:

```text
content/
  getting-started.md
  guides/
    design-system.md
  reference/
    widgets/
      button.md
```

With the default content route, those files produce:

```text
/content/getting-started/
/content/guides/design-system/
/content/reference/widgets/button/
```

Markdown files may contain front matter. Front matter is the page-local metadata block before the Markdown body. It can set metadata, route behavior, and the template for that page.

```markdown
---
title: Getting started
description: Create your first Fission app.
template: crate::site::DocsTemplate
slug: getting-started
section: Guide
---

# Getting started

This page is rendered through the selected template.
```

Template resolution order:

1. `template` in the Markdown front matter when present;
2. `template` on the matching `site.routes` content route;
3. the static site shell documentation template.

Front matter is page data, not executable code. It must be parsed before route rendering so title, description, canonical URL, sitemap settings, social metadata, and template selection are known before HTML generation.

The documentation template is the default template provided by the Fission static site shell. It should render a documentation page shell around the Markdown content: site header, sidebar/tree navigation where configured, table of contents, main article, previous/next links, and metadata-derived head tags. Site owners can replace it globally for a content collection or locally for a single Markdown page.

### 6.2 Template input

Templates receive a typed page model. They must not re-read files from disk or parse arbitrary project state.

```rust
struct StaticMarkdownPage<'a> {
    route: &'a StaticRoute,
    source_path: &'a Path,
    front_matter: &'a FrontMatter,
    body: &'a str,
    markdown: MarkdownRender<'a>,
}

struct MarkdownRender<'a> {
    body: &'a str,
}

impl<'a> MarkdownRender<'a> {
    fn ast(&self) -> anyhow::Result<&'a rushdown::ast::Document>;
    fn widget(&self) -> fission::widgets::MarkdownViewer;
}

trait SiteMarkdownTemplate<S: AppState> {
    type Output: Widget<S>;
    fn render(&self, page: StaticMarkdownPage<'_>, ctx: &StaticBuildCtx) -> Self::Output;
}
```

The Markdown body is stored as a raw string and parsed on demand through a memoized parser service. A template can:

- place `page.markdown.widget()` inside normal Fission layout widgets; or
- inspect `page.markdown.ast()` and render the AST manually with Fission widgets.

The authoritative path still goes through `Widget::build` and Core IR. Raw HTML fragments are not a separate lowering path because they bypass the Fission node model and make validation weaker. If raw HTML in Markdown is enabled, it must be represented by a Fission node that the static renderer can validate before output.

## 7. Output layout

Default output:

```text
dist/site/
  index.html
  content/
    getting-started/
      index.html
  reference/
    widgets/
      button/
        index.html
  assets/
    app.<hash>.css
    site.<hash>.js
    controllers.<hash>.js
    images/...
  sitemap.xml
  robots.txt
```

Each HTML document must be useful without JavaScript. It must include page content, headings, links, images, metadata, canonical URL, and any structured data configured for the route.

## 8. HTML lowering

The static site renderer lowers Fission View/Core IR and Semantics into semantic HTML. It must not lower from a pixel display list. Display lists are for raster/window renderers; the static site target needs structure, semantics, actions, metadata, and style information that would already be lost or flattened in a display list.

Markdown content does not have an HTML bypass. If a template returns a Markdown widget, that widget builds normal Fission nodes; the static renderer then lowers those nodes through the same Core IR visitor as every other widget.

Examples:

| Fission semantics | Static HTML |
| --- | --- |
| heading | `h1`-`h6` based on heading level |
| paragraph/text | `p`, `span`, text nodes |
| link | `a href` |
| button with navigation action | `a` where semantically navigation, otherwise `button` |
| list/list item | `ul`/`ol`/`li` |
| table/grid | `table`, `thead`, `tbody`, `tr`, `th`, `td` where tabular |
| image with label | `img alt` |
| dialog content | static section plus enhancement controller if interactive |
| tabs | static linked sections or enhanced tab controller |
| code block | `pre code` |
| navigation landmark | `nav` |
| main content | `main` |
| complementary content | `aside` |
| footer | `footer` |

The static renderer must not produce generic `div` soup when Semantics contains enough information to emit real HTML elements. If Semantics is missing required information, the build must fail with a useful diagnostic.

## 9. Static support detection

Like the terminal target, static site support must be determined by lowered Core IR plus Semantics, not widget names.

A node is statically lowerable when:

1. it can be represented as semantic HTML and CSS;
2. its state at build time is known;
3. any required client behavior can be represented as a declared enhancement island; and
4. its primary content exists in the HTML before JavaScript runs.

Unsupported without explicit fallback:

- browser-only canvas content that has no static alternate;
- video or audio without static metadata/poster/fallback content;
- live feeds that require runtime network access and have no build-time snapshot;
- user-specific authenticated content;
- widgets that produce primary content only after client JavaScript runs;
- forms with no configured action, mail endpoint, or enhancement behavior;
- interactive components whose default state hides all meaningful content from HTML;
- non-navigation actions that require Rust reducer logic but are not declared as WASM islands.

Build error example:

```text
error[site.primary_content_requires_js]: route primary content is generated only by client behavior
  --> src/site/charts.rs:27:9
   |
27 |         LiveChart::new(metrics_stream)
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = route: /charts/live/
   = reason: static target cannot crawl or index runtime stream data
   = fix: provide a build-time snapshot, static table fallback, or move this route to the web-app target
```

## 10. Progressive enhancement and actions

Static site interactivity must follow progressive enhancement. MDN describes progressive enhancement as providing baseline essential content and functionality first, then adding richer behavior for capable browsers [S5].

Fission should generate static HTML as the baseline and attach browser behavior only to interactive islands. There are four action categories:

1. build-time-only behavior, which does not exist in the generated page;
2. navigation behavior, lowered to real `a href` links or HTML form actions;
3. built-in static enhancements, lowered to small generated or bundled JavaScript controllers with known DOM contracts; and
4. Rust action/reducer behavior, compiled to a route-scoped WASM island rather than translated to JavaScript source.

Fission must not pretend arbitrary Rust reducer logic can be translated into Stimulus controllers. A DOM-controller layer is realistic for built-in behaviors such as tabs, disclosure panels, copy buttons, search boxes, and code playground shells because Fission controls those contracts. It is also realistic for user-supplied JavaScript controllers that are explicitly declared by the site author. It is not realistic as an automatic Rust-to-Stimulus compiler.

Stimulus-style controllers are suitable for the DOM-controller layer because they augment existing HTML through controller, target, action, value, and class attributes instead of owning HTML rendering [S1]. The RFC therefore treats Stimulus as an optional implementation strategy for DOM enhancement, not as the execution model for Rust actions.

Example output:

```html
<section data-controller="fission-tabs">
  <div role="tablist">
    <a href="#install" data-fission-tabs-target="tab">Install</a>
    <a href="#run" data-fission-tabs-target="tab">Run</a>
  </div>

  <section id="install" data-fission-tabs-target="panel">
    <h2>Install</h2>
    <p>Install the CLI and create a project.</p>
  </section>

  <section id="run" data-fission-tabs-target="panel">
    <h2>Run</h2>
    <p>Run the app on your selected target.</p>
  </section>
</section>
```

Without JavaScript, the sections and links still work. With JavaScript, the controller improves keyboard behavior, focus management, and panel switching.

When a static route needs real Rust state transitions after load, the site target should generate a WASM island for that region. The primary HTML still exists before JavaScript runs. The island is responsible only for the interactive region. It receives initial state from the generated HTML or adjacent typed data, handles Fission actions/reducers in Rust, and applies DOM patches through a small browser adapter.

Worker execution is optional and depends on the island. A Web Worker can run Rust/WASM state and reducer logic off the main thread, but it cannot directly manipulate the DOM. If a worker is used, it sends patches or commands to a small main-thread adapter. Simple islands may run WASM on the main thread to avoid worker startup and message-passing overhead.

Unsupported action cases are build errors:

- an interactive button with a non-navigation action and no enhancement strategy;
- a reducer whose state must be preserved but no WASM island was declared;
- a form with no normal action and no configured browser behavior;
- an island whose initial content is empty until WASM runs.

## 11. Navigation enhancement

Navigation enhancement is optional. A static site may use a small browser layer to accelerate link clicks, preserve scroll where appropriate, or replace selected frame content from already-generated HTML. Turbo is one possible implementation because it can accelerate links and forms, decompose pages into frames, and update pages using HTML over the wire [S2]. Fission may use this style of enhancement only when the result remains a multi-page website:

- every route still has its own HTML file;
- every link still has a real `href`;
- every page still works when JavaScript is disabled;
- the browser URL remains the route URL;
- the generated content is not hidden behind a client router;
- forms use normal HTML actions unless explicitly enhanced;
- server-dependent stream behavior is disabled for pure static hosting;
- Rust action/reducer logic is handled by WASM islands, not by navigation enhancement.

Default mode should be:

```toml
[site]
navigation_enhancement = "optional"
```

Allowed values:

```text
off       - plain multi-page static site
optional  - progressive navigation acceleration where safe
frames    - frame-level enhancement for selected regions
```

The static site target must never require navigation enhancement for search-critical content.

## 12. Browser code budget

The site target should produce no JavaScript or WASM unless a route requires enhancement.

Rules:

- no global app runtime by default;
- DOM controllers are tree-shaken by route usage;
- each interactive island declares the controller or WASM module it needs;
- controllers receive data through HTML attributes or adjacent typed data script tags;
- WASM islands are route-scoped or chunked by explicit sharing groups, not shipped as one global app bundle;
- worker-backed islands include a main-thread DOM adapter and a worker module;
- no hidden client-side route manifest for primary navigation unless navigation enhancement is enabled;
- production build reports JS and WASM bytes per route.

Example build report:

```text
Route                              HTML     CSS     JS      WASM
/                                  18 KB    9 KB    0 KB    0 KB
/content/getting-started/          32 KB    9 KB    2 KB    0 KB
/reference/widgets/button/         41 KB    9 KB    3 KB    0 KB
/playground/                       24 KB    9 KB    4 KB    48 KB
```

A route with large browser code must warn unless configured as an interactive documentation/demo route.

## 13. Styling and design systems

The static site renderer must consume the same Fission design-system model as other shells.

Build output:

- extracted CSS custom properties for design tokens;
- route/component CSS generated from Fission styles;
- stable class names in release builds;
- source maps in development builds;
- optional critical CSS inlining;
- dark/light/theme variants expressed with CSS variables and media queries;
- no runtime JSON design-system parsing on the hot path.

The generated CSS should be cacheable and fingerprinted.

## 14. SEO and metadata

Google Search documentation recommends server-side rendering, static rendering, or hydration as long-term solutions for JavaScript-generated content, and notes that JavaScript rendering can have limitations for search crawlers [S4][S6]. Therefore Fission static output must put SEO-critical content directly in HTML.

Each route must support:

- `title`;
- meta description;
- canonical URL;
- language/locale;
- Open Graph metadata;
- social image;
- structured data when configured;
- heading hierarchy validation;
- robots directives;
- sitemap inclusion/exclusion;
- alternate locale links where configured.

The site builder must generate `sitemap.xml` when enabled. Google documents sitemaps as a way to tell crawlers about URLs and when they were updated [S7]. The site builder should also generate `robots.txt` when enabled and include the sitemap URL.

## 15. Data loading

Static site data must be explicit.

```rust
trait StaticDataLoader {
    type Output;

    fn cache_key(&self) -> String;
    fn load(&self, ctx: &StaticBuildCtx) -> anyhow::Result<Self::Output>;
}
```

Rules:

- data loaders run at build time;
- network access is disabled by default in CI unless allowed;
- cache keys are recorded in the site manifest;
- failures are build failures unless a route declares an offline fallback;
- secrets are read through the Fission credential system, not from plaintext config;
- data snapshots can be committed when appropriate.

## 16. Forms

Static forms require explicit behavior.

Allowed forms:

- pure GET forms that navigate to generated routes;
- forms with configured external action URLs;
- forms enhanced by JavaScript controllers;
- forms enhanced by declared WASM islands where Rust logic is required;
- forms that post to a user-configured backend endpoint;
- forms disabled at build time with clear explanatory content.

A form with no action and no enhancement is a build error.

## 17. Search

The static target should support client-side search as a generated asset, not a hosted dependency.

Requirements:

- build local search index during `fission site build`;
- index text from generated HTML;
- support locale-aware indexes;
- write fingerprinted search assets;
- expose search UI as an enhancement island;
- page content remains navigable without search JavaScript.

## 18. Minification and optimization

Production builds should run local optimization steps:

- HTML minification;
- CSS minification;
- JavaScript minification;
- asset fingerprinting;
- image copy/optimization where configured;
- gzip/brotli precompression where configured;
- link and asset integrity checks.

The `minify-html` crate is a candidate for HTML minification because it exposes Rust functions for minifying UTF-8 HTML byte input [S3]. Fission should treat minification as an internal build optimization that can be disabled for debugging.

```toml
[site]
minify = true
precompress = ["gzip", "brotli"]
```

Minification must not change semantics. Site checks should run before and after minification in release builds.

## 19. Development server

`fission site serve` should provide a local development server.

Features:

- serve generated static files;
- rebuild on file change;
- live reload in development only;
- route fallback disabled by default so missing static pages are caught;
- optional preview of production-minified output;
- JSON build diagnostics for editor integrations.

The development server is a local convenience. The production output must be deployable to any static host or object store without the server.

## 20. Validation

`fission site check` must validate:

- every custom route renders;
- every configured Markdown content file produces exactly one route unless explicitly excluded;
- every generated content route renders through its selected template;
- every internal link resolves;
- every image has alt text or explicit decorative status;
- every route has title and meta description;
- canonical URLs match configuration;
- sitemap contains expected routes;
- robots output is valid;
- heading order is sensible;
- no primary content requires JavaScript;
- interactive islands have accessible fallback content;
- Rust action islands have declared WASM output and no missing reducer/action symbols;
- no unsupported Core IR operations remain;
- no broken asset references;
- production minification preserves parseable HTML;
- generated pages work from the filesystem or configured base URL as appropriate.

Errors should use stable IDs:

```text
site.route_not_declared
site.route_render_failed
site.content_route_conflict
site.content_template_missing
site.primary_content_requires_js
site.action_requires_wasm
site.missing_title
site.missing_description
site.missing_alt_text
site.broken_internal_link
site.unsupported_static_op
site.form_missing_action
site.minification_changed_semantics
site.javascript_budget_exceeded
site.browser_code_budget_exceeded
```

## 21. Deployment

Static site deployment should reuse the post-build distribution system.

```text
fission package --target static-site --format static --release
fission distribute --provider s3 --artifact target/fission/release/static-site/static/artifact-manifest.json
fission distribute --provider google-drive --artifact target/fission/release/static-site/static/artifact-manifest.json
fission distribute --provider onedrive --artifact target/fission/release/static-site/static/artifact-manifest.json
fission distribute --provider dropbox --artifact target/fission/release/static-site/static/artifact-manifest.json
```

The artifact manifest must include:

- generated HTML files;
- CSS/JS assets;
- static assets;
- sitemap and robots files;
- content hashes;
- MIME types;
- compression variants;
- route manifest;
- build metadata.

## 22. Testing

Required tests:

- route enumeration tests;
- Markdown content discovery and front matter tests;
- template selection tests;
- HTML lowering tests for Semantics-to-element mapping;
- unsupported widget/static-op verifier tests;
- action classification and WASM-island verifier tests;
- metadata generation tests;
- sitemap and robots tests;
- link checker tests;
- no-JavaScript baseline tests;
- enhancement island tests;
- minification equivalence tests;
- full site smoke build for Fission's own website/docs.

A no-JavaScript baseline test must parse generated HTML and confirm meaningful route content exists without executing scripts.

## 23. Acceptance criteria

The static site target is accepted when:

- `fission add-target site` configures the target without changing app runtime code.
- `fission site build --release` emits a multi-page static site with one HTML document per custom route and generated content route.
- `content/` Markdown files generate routes using the static site shell documentation template by default.
- Markdown front matter can override metadata and page template.
- generated pages contain primary content before JavaScript runs.
- generated routes have titles, descriptions, canonical URLs, and sitemap entries where configured.
- interactive islands work with JavaScript and remain readable without JavaScript.
- Rust action/reducer islands run through generated WASM/browser adapters rather than attempted Rust-to-JavaScript translation.
- unsupported static rendering fails the build with source provenance.
- production output is minified locally when enabled.
- no hosted service is required to build or deploy the site.
- Fission's own website/docs can be generated by this target.

## References

[S1] Stimulus documentation: https://stimulus.hotwired.dev/  
[S2] Turbo documentation: https://turbo.hotwired.dev/  
[S3] `minify-html` crate documentation: https://docs.rs/minify-html/latest/minify_html/  
[S4] Google Search Central, Dynamic Rendering as a workaround: https://developers.google.com/search/docs/crawling-indexing/javascript/dynamic-rendering  
[S5] MDN, Progressive enhancement: https://developer.mozilla.org/docs/Glossary/Progressive_Enhancement  
[S6] Google Search Central, JavaScript SEO basics: https://developers.google.com/search/docs/crawling-indexing/javascript/javascript-seo-basics  
[S7] Google Search Central, Build and submit a sitemap: https://developers.google.com/search/docs/crawling-indexing/sitemaps/build-sitemap
