//! Static site shell for Fission.
//!
//! The site shell renders real Fission widget output. Applications and generated
//! content are built into `Node` trees, lowered to Core IR, and then visited by
//! the HTML renderer. This crate does not provide an HTML-first page model or a
//! replacement router.

mod build;
mod document;
mod front_matter;
mod html;
mod search;
mod site;

pub use build::{
    build_content_site, build_site, check_content_site, check_site, list_content_routes,
    list_site_routes, SiteBuildOptions, SiteBuildReport, SiteContentRouteConfig, SiteRouteReport,
};
pub use html::{render_ir_to_html, CodeHighlightingOptions, HtmlRenderOptions, RenderedHtml};
pub use site::{
    build_from_cli, ContentTransform, CustomRoute, FissionSite, SitePageElement,
    SitePageElementFilter, SitePageElementPlacement, SiteRenderContext,
};
