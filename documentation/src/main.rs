mod charts;
mod components;
mod registry;

use anyhow::Result;
use components::{
    CrateDetailPage, CrateDirectoryPage, DocsFooter, DocsState, LocalizedLandingPage,
    MarketingPageKind, ProductMarketingPage, RoutedHomePage,
};
use fission::prelude::*;
use fission::site::{build_from_cli, FissionSite};

fn main() -> Result<()> {
    build_from_cli(site_app())
}

fn site_app() -> FissionSite {
    let registry_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/fission-crates.sqlite3");
    let registry = registry::load_registry(&registry_path).unwrap_or_else(|error| {
        eprintln!("crate registry unavailable: {error:#}");
        Vec::new()
    });
    let mut env = Env::default();
    env.i18n
        .add_bundle(load_bundle("en-GB", include_str!("../i18n/en-GB.json")));
    env.i18n
        .add_bundle(load_bundle("es-ES", include_str!("../i18n/es-ES.json")));
    let mut site = FissionSite::new()
        .with_env(env)
        .light_dark_themes(Theme::default(), Theme::dark(), DesignMode::Light)
        .route_widget::<DocsState, _>(
            "/",
            "Fission",
            Some(
                "Build, test, package, and release production Rust apps across macOS, Windows, Linux, Web, Android, iOS, Terminal, Static site, and SSR targets."
                    .to_string(),
            ),
            RoutedHomePage::new("/"),
        )
        .route_widget::<DocsState, _>(
            "/es/",
            "Fission",
            Some("Crea aplicaciones Rust para todas las plataformas.".to_string()),
            LocalizedLandingPage,
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
            Some("Build across macOS, Windows, Linux, Web, Android, and iOS from one shared Rust application model.".to_string()),
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
            Some("Generate SEO-friendly Static site targets from Fission widgets, Markdown content, and explicit site routing.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::StaticSites),
        )
        .route_widget::<DocsState, _>(
            "/product/server-rendered-sites/",
            "Server-rendered sites",
            Some("Render dynamic HTML with Fission widgets, server jobs, signed actions, route caching, workers, and focused islands.".to_string()),
            ProductMarketingPage::new(MarketingPageKind::ServerSites),
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
        .route_widget::<DocsState, _>(
            "/crates/",
            "Fission crates",
            Some("Libraries and tools built directly on the Fission framework.".to_string()),
            CrateDirectoryPage::new(registry.clone()),
        )
        .route_widget::<DocsState, _>(
            "/es/crates/",
            "Crates de Fission",
            Some("Bibliotecas y herramientas creadas directamente sobre Fission.".to_string()),
            CrateDirectoryPage::new(registry.clone()),
        )
        .footer_widget::<DocsState, _>(DocsFooter)
        .user_css(include_str!("../site/overrides.css"))
        .content_transform(charts::expand_documentation_mdx);
    for item in registry {
        let path = format!("/crates/{}/", item.name);
        let title = format!("{} — Fission crates", item.name);
        let description = Some(item.description.clone());
        site =
            site.route_widget::<DocsState, _>(path, title, description, CrateDetailPage::new(item));
    }
    site
}

fn load_bundle(locale: &str, json: &str) -> fission::i18n::TranslationBundle {
    fission::i18n::TranslationBundle {
        locale: locale.into(),
        messages: serde_json::from_str(json).expect("valid checked-in translation bundle"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn content_code_fences_do_not_swallow_markdown_sections() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("content");
        let mut files = Vec::new();
        collect_markdown_files(&root, &mut files);

        let mut failures = Vec::new();
        for path in files {
            let source = fs::read_to_string(&path).expect("read documentation file");
            let mut in_fence = false;
            let mut fence_start = 0usize;
            let mut fence_lang = String::new();
            for (index, line) in source.lines().enumerate() {
                let line_number = index + 1;
                if line.starts_with("```") {
                    if in_fence {
                        in_fence = false;
                        fence_lang.clear();
                    } else {
                        in_fence = true;
                        fence_start = line_number;
                        fence_lang = line
                            .trim()
                            .strip_prefix("```")
                            .unwrap_or_default()
                            .split_whitespace()
                            .next()
                            .unwrap_or_default()
                            .to_ascii_lowercase();
                    }
                    continue;
                }

                if in_fence
                    && !matches!(fence_lang.as_str(), "md" | "mdx" | "markdown")
                    && looks_like_markdown_section(line)
                {
                    failures.push(format!(
                        "{}:{line_number} is inside non-Markdown fence opened at line {fence_start}",
                        path.display()
                    ));
                }
            }
            if in_fence {
                failures.push(format!(
                    "{}:{fence_start} opens a code fence that is never closed",
                    path.display()
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "documentation contains malformed fenced blocks:\n{}",
            failures.join("\n")
        );
    }

    fn collect_markdown_files(dir: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read documentation content directory") {
            let entry = entry.expect("read documentation content entry");
            let path = entry.path();
            if path.is_dir() {
                collect_markdown_files(&path, files);
            } else if matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("md" | "mdx")
            ) {
                files.push(path);
            }
        }
    }

    fn looks_like_markdown_section(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("# ")
            || trimmed.starts_with("## ")
            || trimmed.starts_with("### ")
            || trimmed.starts_with("#### ")
            || trimmed.starts_with("##### ")
            || trimmed.starts_with("###### ")
            || trimmed.starts_with("| ---")
            || trimmed.starts_with("|---")
            || trimmed.starts_with("| :---")
            || trimmed.starts_with("|:---")
    }
}
