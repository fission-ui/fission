mod charts;
mod components;

use anyhow::Result;
use components::{DocsFooter, DocsState, MarketingPageKind, ProductMarketingPage, RoutedHomePage};
use fission::prelude::*;
use fission::site::{build_from_cli, FissionSite};

fn main() -> Result<()> {
    build_from_cli(site_app())
}

fn site_app() -> FissionSite {
    FissionSite::new()
        .light_dark_themes(Theme::default(), Theme::dark(), DesignMode::Dark)
        .route_widget::<DocsState, _>(
            "/",
            "Fission",
            Some(
                "Build, test, package, and release production Rust apps across desktop, mobile, web, terminal, and static site targets."
                    .to_string(),
            ),
            RoutedHomePage::new("/"),
        )
        .route_widget::<DocsState, _>(
            "/product/overview/",
            "Fission platform",
            Some("A Rust application platform for the full product lifecycle.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::Overview),
        )
        .route_widget::<DocsState, _>(
            "/product/cross-platform-apps/",
            "Cross-platform apps",
            Some("Build desktop, mobile, and web apps from one shared Rust application model.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::CrossPlatformApps),
        )
        .route_widget::<DocsState, _>(
            "/product/terminal-apps/",
            "Terminal apps",
            Some("Build terminal user interfaces with the same Fission app model used for graphical apps.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::TerminalApps),
        )
        .route_widget::<DocsState, _>(
            "/product/static-sites/",
            "Static sites",
            Some("Generate SEO-friendly static HTML sites from Fission widgets, Markdown content, and explicit site routing.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::StaticSites),
        )
        .route_widget::<DocsState, _>(
            "/product/production-lifecycle/",
            "Production lifecycle",
            Some("Package, sign, release, distribute, and track production Fission apps.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::ProductionLifecycle),
        )
        .route_widget::<DocsState, _>(
            "/product/developer-tools/",
            "Developer tools",
            Some("Developer tools for inspection, diagnostics, profiling, screenshots, device workflow, and IDE integration.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::DeveloperTools),
        )
        .route_widget::<DocsState, _>(
            "/product/design-systems/",
            "Design systems",
            Some("Use design system package JSON to generate typed Fission theme code.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::DesignSystems),
        )
        .route_widget::<DocsState, _>(
            "/product/charts/",
            "Charts and data visualization",
            Some("Native Fission charts for dashboards, analytics, finance, maps, networks, dynamic data, and 3D-ready visuals.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::Charts),
        )
        .footer_widget::<DocsState, _>(DocsFooter)
        .user_css(include_str!("../site/overrides.css"))
        .content_transform(charts::expand_documentation_mdx)
}
