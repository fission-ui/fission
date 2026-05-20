mod charts;

use anyhow::Result;
use fission::site::{
    build_from_cli, markdown_to_html_with_route, marketing_page, HtmlPage, PageRenderContext,
    PageShell, Router, StaticMarkdownPage, StaticMarkdownTemplate, StaticPage, StaticSiteApp,
};

fn main() -> Result<()> {
    build_from_cli(site_app())
}

fn site_app() -> StaticSiteApp {
    StaticSiteApp::new(Router::new().route("/", HomePage))
        .template("fission::site::DocumentationTemplate", DocsTemplate)
}

#[derive(Clone, Debug)]
struct HomePage;

impl StaticPage for HomePage {
    fn render(&self, _ctx: &PageRenderContext<'_>) -> Result<HtmlPage> {
        Ok(marketing_page(
            "Fission",
            "Build production desktop, web, Android, and iOS apps with one Rust UI framework.",
            HOME_HTML,
        ))
    }
}

#[derive(Clone, Debug)]
struct DocsTemplate;

impl StaticMarkdownTemplate for DocsTemplate {
    fn render(
        &self,
        page: StaticMarkdownPage<'_>,
        ctx: &PageRenderContext<'_>,
    ) -> Result<HtmlPage> {
        let body = charts::expand_documentation_mdx(page.body, ctx.project_dir)?;
        let rendered = markdown_to_html_with_route(&body, page.route_path);
        Ok(HtmlPage {
            title: page
                .front_matter
                .title()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "Documentation".to_string()),
            description: page
                .front_matter
                .description()
                .map(ToOwned::to_owned)
                .unwrap_or_default(),
            body: rendered.html,
            template: PageShell::Documentation,
        })
    }
}

const HOME_HTML: &str = r##"
<section class="hero">
  <div class="hero-pill">Production Rust UI for every target family</div>
  <h1>Build serious apps with one Rust interface model.</h1>
  <p class="lead">Fission is a production-ready UI framework for desktop, web, Android, and iOS. Your state, reducers, layout, semantics, rendering, diagnostics, and tests stay in one coherent Rust architecture.</p>
  <div class="hero-actions">
    <a class="btn primary" href="/docs/learn/quickstart/">Start building</a>
    <a class="btn secondary" href="/docs/learn/runtime-model/">Understand the model</a>
  </div>
  <div class="command-grid">
    <div class="command-card"><strong>Create</strong><br /><code>cargo fission init my-app</code></div>
    <div class="command-card"><strong>Run</strong><br /><code>cargo fission run --target web</code></div>
  </div>
</section>

<section class="section">
  <div class="section-inner">
    <h2>A framework for teams who need native-grade products, not throwaway demos.</h2>
    <p class="section-lead">Fission is designed around predictable state transitions, typed actions, explicit host work, inspectable layout, and platform shells that keep the same app model alive across targets.</p>
    <div class="signal-grid">
      <article class="signal-card"><div class="signal-icon">01</div><h3>One shared runtime</h3><p>State, reducers, layout, semantics, rendering, and diagnostics stay in one app model.</p><a href="/docs/learn/runtime-model/">See the model</a></article>
      <article class="signal-card"><div class="signal-icon">02</div><h3>Real target families</h3><p>Desktop, web, Android, and iOS shells host the same Rust application code.</p><a href="/docs/learn/examples-and-targets/">See targets</a></article>
      <article class="signal-card"><div class="signal-icon">03</div><h3>Built for verification</h3><p>Live tests, diagnostics, semantics, and layout inspection are part of the product workflow.</p><a href="/docs/guides/testing-and-diagnostics/">See testing</a></article>
      <article class="signal-card"><div class="signal-icon">04</div><h3>CLI-led workflow</h3><p>Project setup, target generation, device selection, and smoke tests are exposed through the Fission CLI.</p><a href="/docs/guides/platform-shells-cli-and-testing/">See the CLI</a></article>
    </div>
  </div>
</section>

<section class="section architecture">
  <div class="section-inner">
    <h2>The app flow stays explicit from input to pixels.</h2>
    <p class="section-lead">Fission keeps the important parts of a product easy to reason about: data is plain Rust, changes have named causes, outside work has an explicit path, and rendering stays inspectable.</p>
    <div class="arch-steps">
      <div class="arch-step"><span>01</span><h3>State</h3><p>Plain Rust data holds product truth instead of hiding it in widgets or host callbacks.</p></div>
      <div class="arch-step"><span>02</span><h3>Reducers</h3><p>Typed actions describe user intent and reducers make durable state changes reviewable.</p></div>
      <div class="arch-step"><span>03</span><h3>Host work</h3><p>Files, timers, services, capabilities, and background jobs use explicit runtime paths.</p></div>
      <div class="arch-step"><span>04</span><h3>Render</h3><p>Layout, semantics, paint order, and diagnostics remain available for tests and debugging.</p></div>
    </div>
  </div>
</section>

<section class="section">
  <div class="section-inner">
    <h2>Ship across desktop, web, Android, and iOS.</h2>
    <p class="section-lead">Fission's shells are platform-specific where they need to be, but the application model stays shared. That is the difference between porting a product and rebuilding it four times.</p>
    <div class="target-grid">
      <article class="target-card"><h3>Desktop</h3><p>Windows, macOS, and Linux shells for local development and production desktop apps.</p><a href="/docs/learn/examples-and-targets/">Desktop path</a></article>
      <article class="target-card"><h3>Web</h3><p>Browser-hosted Fission apps with generated web target scaffolding and smoke tests.</p><a href="/docs/guides/platform-shells-cli-and-testing/">Web path</a></article>
      <article class="target-card"><h3>Android</h3><p>Android target scaffolding, emulator workflow, device logs, and CLI-driven runs.</p><a href="/docs/guides/platform-shells-cli-and-testing/">Android path</a></article>
      <article class="target-card"><h3>iOS</h3><p>iOS simulator workflow, bundle generation, test control, and platform integration.</p><a href="/docs/guides/platform-shells-cli-and-testing/">iOS path</a></article>
    </div>
  </div>
</section>

<section class="section architecture">
  <div class="section-inner">
    <h2>Beautiful charts belong in the framework.</h2>
    <p class="section-lead">Fission Charts is built as a first-class visualization layer for product dashboards, analytics tools, operations consoles, and interactive reports.</p>
    <div class="chart-strip">
      <article class="chart-card"><div class="chart-preview"><span></span><span></span><span></span></div><h3>Cartesian and radial</h3><p>Line, bar, area, pie, radar, gauge, and polar families.</p></article>
      <article class="chart-card"><div class="chart-preview"><span></span><span></span><span></span></div><h3>Operations and analytics</h3><p>Heatmaps, timelines, funnels, sankey, treemaps, graph, and monitoring views.</p></article>
      <article class="chart-card"><div class="chart-preview"><span></span><span></span><span></span></div><h3>3D and GL</h3><p>Surface, globe, scatter3D, graph3D, terrain, mesh, and point-cloud catalog entries.</p></article>
    </div>
  </div>
</section>

<section class="section">
  <div class="section-inner">
    <h2>Start with the hand-held path, then go deeper.</h2>
    <p class="section-lead">The documentation is written for readers who may be new to Rust UI work. It introduces each concept before relying on it, then backs the explanation with examples and reference pages.</p>
    <div class="hero-actions" style="justify-content:flex-start">
      <a class="btn primary" href="/docs/intro/">Read the docs</a>
      <a class="btn secondary" href="/reference/overview/overview/">Open the reference</a>
    </div>
  </div>
</section>
"##;
