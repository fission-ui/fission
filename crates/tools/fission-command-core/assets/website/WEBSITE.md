# Work on this website

Immediately read the generated `AGENTS.md` and its linked guidance after `fission init`.
It lives at the nearest enclosing Git root, or the app root outside Git. If that root
already has user instructions, read them and `AGENTS.fission.md` beside them.
Preserve user-authored instructions. Read the [custom page guide](https://fission.rs/docs/guides/static-site-custom-pages/)
and [responsive page guide](https://fission.rs/docs/cookbook/responsive-static-site-page/).

Edit Rust under `src/`, translations under `i18n/`, design tokens under `design/`,
and source assets under `assets/`. Fission generates the HTML and WASM host: never
edit generated output to change the UI. Keep `fission.toml` and the generated
target/dependency organization. `fission init PATH --website-template` selects
this source template and configures only `static-site`; it takes no target value.
Plain `fission init` keeps its existing application scaffold and target defaults.
Add Web through the normal target command:

```sh
fission add-target web --project-dir .
```

That command updates the manifest, dependency features and platform files while
preserving the shared source, including its existing WASM entrypoint. Other
`add-target` targets use the same CLI path; implement the corresponding thin shell
entrypoint and inspect `platforms/<target>/README.md` before running them. This
template supplies static and Web entrypoints, not native/mobile/SSR/backend ones.

Generated source tree (platform files for Web appear only after adding Web):

```text
Cargo.toml, fission.toml, build.rs, README.md, WEBSITE.md
src/main.rs       static shell entrypoint
src/lib.rs        static route registration and Web shell entrypoint
src/state.rs      shared state and Web route reducer
src/page.rs       reusable home/about page composition
src/navigation.rs, src/footer.rs
src/design.rs    theme and embedded translation setup
design/dsp.json, design/tokens.json
i18n/en.json, i18n/es.json
assets/app-icon.png
platforms/site/README.md
```

`src/page.rs` contains the home/about pages; `src/navigation.rs` and `src/footer.rs`
are reusable components. `design/dsp.json` inherits Fission's default component
recipes and generates a typed theme from the checked-in `design/tokens.json`.
English and Spanish bundles are embedded in `src/design.rs`; English is the
initial locale. Check longer translated labels when changing content.
For a new page, extend `Page` and the shared page component, register the static
route in `build_site`, and add its route to the Web `Router` in `src/lib.rs`.
Update navigation, translations and semantic identifiers together. Keep app
behavior in shared modules and shell entrypoints thin. `site.routes = []` disables
implicit Markdown discovery; opt into content mounts deliberately when adding
Markdown pages. Do not regenerate the project to add pages or targets.

## Build, serve, run and test

From the project root:

```sh
cargo fmt --all -- --check
fission site routes --project-dir .
fission site check --project-dir .
fission site build --project-dir .
fission site serve --project-dir . --host 127.0.0.1 --port 8123
# The normal run path also serves the configured Static site target:
fission run --target static-site --project-dir . --host 127.0.0.1 --port 8123 --no-open
fission test --target static-site --project-dir .
```

Visit `/` and `/about/`. Click navigation and the page link, test keyboard focus
and activation, open `/about/` directly, then refresh. Inspect screenshots at
about 390px and 1280px; fix horizontal overflow, clipped text, and inaccessible
controls before calling the website finished. Stable controls are `nav-home`,
`nav-about`, `page-next`, and `fission-attribution`. Responsive branches share
identifiers; browser tests must select the visible branch.

After `add-target web`, first install `wasm32-unknown-unknown` and `wasm-pack`,
then use the normal Web lifecycle:

```sh
fission doctor web --project-dir .
fission build --target web --project-dir .
fission run --target web --project-dir . --host 127.0.0.1 --port 8124 --no-open
fission test --target web --project-dir .
```

The target tests are smoke checks (build/render or browser launch and runtime
health), not proof of every route or control. For this custom static entrypoint,
the current `test --target static-site` path builds it and asks you to run
project-specific browser coverage; it does not automatically exercise its routes.
Add Rust tests for your state and
reducers and browser/LiveTest coverage for the flows you implement. Run `cargo
test` as appropriate for your source modules, and exercise navigation manually
as described below. A missing toolchain/browser is a setup gap reported by
`doctor`, not a successful test.

Confirm `pkg/` JavaScript glue and WASM load, inspect the rendered app, and test
home → about → back/forward. This starter uses Fission hash routing: `/#/about/`
(or `/repository-name/#/about/`) supports direct visits and refresh on static
hosts. The Web entrypoint seeds the shell's existing navigation base path from
the current host directory, so links retain a project mount without hard-coding
a repository name. Plain `/about/` is the separate static page, not a Web application route.
Do not switch to pathname routing on Pages without an intentional fallback;
Pages cannot provide the local Web preview server's extensionless-route fallback.

## Prepare for GitHub Pages

Pages serves static files. It does not host a backend, database, secrets, or
server-side form handling. These pages contain honest placeholders and no live
integrations. For the free Fission Pages workflow, retain a visible
`Built with Fission` link to `https://fission.rs` on every page; remove it only
with verified paid entitlement and a user request.

```sh
fission package --target static-site --format static --project-dir .
# For client-side interaction, package the Web target instead:
fission package --target web --format static --project-dir .
```

The CLI prints the artifact manifest and package directory. Preview the selected
package unchanged at both `/` and `/repository-name/` using a local static server
(mount a copy of the package in that directory under the server root). Check every
page, internal link, favicon, stylesheet, runtime asset, and Web WASM request.
Static links are route-relative, and Web links use hashes; neither needs a hard-coded
repository name. Avoid adding root-absolute asset URLs when customizing the site.

For a publication plan, minimally configure the existing generated `fission.toml`:

```toml
[distribution.github_pages.production]
owner = "YOUR_GITHUB_OWNER"
repo = "YOUR_REPOSITORY_NAME"
base_path = "/YOUR_REPOSITORY_NAME/"
```

Those uppercase strings are placeholders. Set `base_path = "/"` for a user/org
site or custom domain. A real `[site].base_url` is optional canonical metadata;
it is not an asset/link rewrite. Review the existing tooling without publishing:

```sh
fission distribute setup --provider github-pages --project-dir . --dry-run
fission publish --provider github-pages --target static-site --project-dir . --dry-run
# Or, for the client-side package:
fission publish --provider github-pages --target web --project-dir . --dry-run
```

The setup preview uses the existing static-site Actions workflow. A Web artifact
must be uploaded from its Web package directory; do not accidentally deploy the
static package. Review the workflow's Fission CLI installation and artifact path
before use. Publishing, enabling Pages, and configuring a domain require a separate
explicit request. Preparation commands above do not enable Pages or deploy a site.
The publish dry-run will report a missing workflow until a reviewed workflow has
been saved; setup dry-run previews it without creating that file.
