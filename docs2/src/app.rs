use crate::content::{load_goal, GoalDocument};
use crate::design::DocsDesignSystem;
use crate::i18n;
use crate::screens::{DocumentationHub, GoalPage, HomePage};
use crate::state::DocsState;
use anyhow::Result;
use fission::prelude::{DesignMode, DesignSystem};
use fission::site::{FissionSite, WebFontDelivery};
use std::path::Path;

pub fn site_app() -> Result<FissionSite> {
    let project_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let goal = load_goal(project_dir.join("content/goals/interactive-counter.mdx"))?;
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
            HomePage::new(),
        )
        .with_route_structured_data("/", homepage_structured_data())
        .route_widget::<DocsState, _>(
            "/docs/",
            "Documentation — Fission",
            Some("Learn to build applications with Fission through tutorials and reference material.".to_string()),
            DocumentationHub::new(),
        )
        .route_widget::<DocsState, _>(
            goal.meta.path.clone(),
            format!("{} — Fission Rust GUI tutorial", goal.meta.title),
            Some(goal.meta.description.clone()),
            GoalPage::new(goal.clone()),
        )
        .with_route_structured_data(goal.meta.path.clone(), goal_structured_data(&goal));

    for alias in &goal.meta.aliases {
        site = site.route_widget::<DocsState, _>(
            alias.clone(),
            goal.meta.title.clone(),
            Some(goal.meta.description.clone()),
            GoalPage::new(goal.clone()),
        );
    }
    Ok(site)
}

fn homepage_structured_data() -> [String; 1] {
    [serde_json::json!({
        "@context": "https://schema.org",
        "@type": "SoftwareApplication",
        "name": "Fission",
        "applicationCategory": "DeveloperApplication",
        "operatingSystem": "macOS, Windows, Linux, Android, iOS and Web",
        "description": "A Rust GUI framework for building desktop, mobile and web apps from one shared product model.",
        "license": "https://www.apache.org/licenses/LICENSE-2.0",
        "url": "https://fission.rs/"
    })
    .to_string()]
}

fn goal_structured_data(goal: &GoalDocument) -> [String; 1] {
    [serde_json::json!({
        "@context": "https://schema.org",
        "@type": "HowTo",
        "name": goal.meta.title.clone(),
        "description": goal.meta.description.clone(),
        "totalTime": "PT10M",
        "step": goal.steps.iter().map(|step| serde_json::json!({
            "@type": "HowToStep",
            "name": step.title.clone(),
            "url": format!("https://fission.rs{}#{}", goal.meta.path, step.id)
        })).collect::<Vec<_>>()
    })
    .to_string()]
}
