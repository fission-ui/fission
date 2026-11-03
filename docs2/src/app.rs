use crate::content::{load_goals, GoalDocument, GoalMeta};
use crate::design::DocsDesignSystem;
use crate::i18n;
use crate::navigation;
use crate::products::{product_routes, ProductRoute};
use crate::screens::{DocumentationHub, GoalPage, HomePage, TutorialsIndexPage};
use crate::state::DocsState;
use anyhow::{Context, Result};
use fission::prelude::{DesignMode, DesignSystem};
use fission::site::{FissionSite, WebFontDelivery};
use std::path::Path;

const HOMEPAGE_PREVIEW_APP: &str = "single-page-web-app-rust";

pub fn site_app() -> Result<FissionSite> {
    let project_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let goals = load_goals(project_dir.join("content/goals"))?;
    let tutorials: Vec<GoalMeta> = goals.iter().map(|goal| goal.meta.clone()).collect();
    let hero_tutorial = goals
        .iter()
        .find(|goal| goal.meta.app == HOMEPAGE_PREVIEW_APP)
        .context("homepage preview tutorial is missing")?;
    let home_preview = hero_tutorial
        .steps
        .first()
        .context("homepage preview tutorial has no first step")?;
    let hero_source = home_preview
        .files
        .get("src/app.rs")
        .context("homepage preview step is missing src/app.rs")?
        .clone();
    let hero_preview_url = home_preview.preview_url(&hero_tutorial.meta.app);
    let products = product_routes();
    let mut site = FissionSite::new()
        .with_env(i18n::environment())
        .with_design_system::<DocsDesignSystem>(DesignMode::Light)
        .light_dark_themes(
            DocsDesignSystem::theme(DesignMode::Light),
            DocsDesignSystem::theme(DesignMode::Dark),
            DesignMode::Light,
        )
        .with_font_delivery(WebFontDelivery::External)
        .default_locale("en-GB")
        .route_widget::<DocsState, _>(
            "/",
            "Fission — Rust GUI framework for desktop, mobile and web",
            Some("Build native desktop, mobile and web applications in Rust from one shared product model with Fission.".to_string()),
            HomePage::new(tutorials.clone(), hero_source, hero_preview_url),
        )
        .with_route_structured_data("/", homepage_structured_data())
        .route_widget::<DocsState, _>(
            "/docs/",
            "Documentation — Fission",
            Some("Learn to build applications with Fission through tutorials and reference material.".to_string()),
            DocumentationHub::new(tutorials.clone()),
        )
        .route_widget::<DocsState, _>(
            navigation::TUTORIALS,
            "Fission tutorials: build Rust apps and websites",
            Some("Step-by-step Fission tutorials for building complete Rust apps and websites, from the first project command to a tested, deployable result.".to_string()),
            TutorialsIndexPage::new(tutorials.clone()),
        )
        .with_route_structured_data(navigation::TUTORIALS, tutorials_structured_data(&goals));

    for goal in goals {
        let path = goal.meta.path.clone();
        site = site
            .route_widget::<DocsState, _>(
                path.clone(),
                format!("{} — Fission tutorial", goal.meta.title),
                Some(goal.meta.description.clone()),
                GoalPage::new(goal.clone(), tutorials.clone()),
            )
            .with_route_structured_data(path, goal_structured_data(&goal));

        for alias in &goal.meta.aliases {
            site = site.route_widget::<DocsState, _>(
                alias.clone(),
                goal.meta.title.clone(),
                Some(goal.meta.description.clone()),
                GoalPage::new(goal.clone(), tutorials.clone()),
            );
        }
    }

    for product in &products {
        let path = product.path;
        site = site
            .route_widget::<DocsState, _>(
                path,
                product.title,
                Some(product.description.to_string()),
                crate::screens::ProductPage::new(product.clone(), products.clone()),
            )
            .with_route_structured_data(path, product_structured_data(product));
    }
    Ok(site)
}

fn product_structured_data(product: &ProductRoute) -> [String; 1] {
    [serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebPage",
        "name": product.title,
        "description": product.description,
        "url": format!("https://fission.rs{}", product.path),
        "about": {
            "@type": "SoftwareApplication",
            "name": "Fission",
            "applicationCategory": "DeveloperApplication",
            "operatingSystem": "macOS, Windows, Linux, Android, iOS, Web, Terminal, Static site and SSR"
        }
    })
    .to_string()]
}

fn homepage_structured_data() -> [String; 1] {
    [serde_json::json!({
        "@context": "https://schema.org",
        "@type": "SoftwareApplication",
        "name": "Fission",
        "applicationCategory": "DeveloperApplication",
        "operatingSystem": "macOS, Windows, Linux, Android, iOS, Web, Terminal, Static site and SSR",
        "description": "A Rust GUI framework for building desktop, mobile and web apps from one shared product model.",
        "license": "https://www.apache.org/licenses/LICENSE-2.0",
        "url": "https://fission.rs/"
    })
    .to_string()]
}

fn tutorials_structured_data(goals: &[GoalDocument]) -> [String; 1] {
    [serde_json::json!({
        "@context": "https://schema.org",
        "@type": "CollectionPage",
        "name": "Fission tutorials: build Rust apps and websites",
        "description": "Step-by-step Fission tutorials for building complete Rust apps and websites.",
        "url": format!("https://fission.rs{}", navigation::TUTORIALS),
        "mainEntity": {
            "@type": "ItemList",
            "itemListElement": goals.iter().enumerate().map(|(index, goal)| serde_json::json!({
                "@type": "ListItem",
                "position": index + 1,
                "name": goal.meta.title.clone(),
                "url": format!("https://fission.rs{}", goal.meta.path)
            })).collect::<Vec<_>>()
        }
    })
    .to_string()]
}

fn goal_structured_data(goal: &GoalDocument) -> [String; 1] {
    [serde_json::json!({
        "@context": "https://schema.org",
        "@type": "HowTo",
        "name": goal.meta.title.clone(),
        "description": goal.meta.description.clone(),
        "totalTime": duration_iso8601(&goal.meta),
        "step": goal.steps.iter().map(|step| serde_json::json!({
            "@type": "HowToStep",
            "name": step.title.clone(),
            "url": format!("https://fission.rs{}#{}", goal.meta.path, step.id)
        })).collect::<Vec<_>>()
    })
    .to_string()]
}

fn duration_iso8601(meta: &GoalMeta) -> Option<String> {
    let normalized = meta.duration.trim().to_ascii_lowercase();
    let (value, unit) = normalized.split_once(' ')?;
    let value = value.parse::<u32>().ok()?;
    let suffix = match unit.trim_end_matches('s') {
        "minute" | "min" => "M",
        "hour" | "hr" => "H",
        _ => return None,
    };
    Some(format!("PT{value}{suffix}"))
}
