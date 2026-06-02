//! Static site shell for Fission.
//!
//! The site shell renders real Fission widget output. Applications and generated
//! content are built into `Widget` trees, lowered to Core IR, and then visited by
//! the HTML renderer. This crate does not provide an HTML-first page model or a
//! replacement router.

mod browser_island;
mod build;
mod document;
mod front_matter;
mod html;
mod search;
mod site;

pub use browser_island::{run_browser_island, BrowserIslandApp};
pub use build::{
    build_content_site, build_site, check_content_site, check_site, list_content_routes,
    list_site_routes, site_base_css, site_enhancement_js, SiteBuildOptions, SiteBuildReport,
    SiteContentRouteConfig, SiteRouteReport,
};
pub use document::SiteNavLink;
pub use html::{
    render_ir_to_html, render_ir_to_html_with_styles, theme_variables_css, CodeHighlightingOptions,
    CssVariableMap, HtmlRenderOptions, RenderedHtml, StyleRegistry,
};
pub use site::{
    build_from_cli, ContentTransform, CustomRoute, FissionSite, SitePageElement,
    SitePageElementFilter, SitePageElementPlacement, SiteRenderContext,
};
