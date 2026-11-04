use crate::content::{load_goals, GoalDocument, GoalMeta};
use crate::design::DocsDesignSystem;
use crate::i18n;
use crate::navigation;
use crate::products::{product_routes, ProductRoute};
use crate::screens::{
    tutorial_categories, DocumentationHub, GoalPage, HomePage, PlaygroundExample, PlaygroundPage,
    TutorialCategoryPage, TutorialsIndexPage, TUTORIALS_PER_PAGE,
};
use crate::state::DocsState;
use anyhow::{Context, Result};
use fission::prelude::{DesignMode, DesignSystem};
use fission::site::FissionSite;
use std::path::Path;

const HOMEPAGE_PREVIEW_APP: &str = "single-page-web-app-rust";

pub fn site_app() -> Result<FissionSite> {
    let project_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let goals = load_goals(project_dir.join("content/goals"))?;
    let tutorials: Vec<GoalMeta> = goals.iter().map(|goal| goal.meta.clone()).collect();
    let categories = tutorial_categories(&tutorials);
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
            TutorialsIndexPage::new(categories.clone()),
        )
        .with_route_structured_data(navigation::TUTORIALS, tutorials_structured_data(&categories));

    for category in &categories {
        for page in 1..=category.page_count() {
            let path = category.route(page);
            let page_label = if category.page_count() > 1 {
                format!(" — page {page}")
            } else {
                String::new()
            };
            site = site
                .route_widget::<DocsState, _>(
                    path.clone(),
                    format!(
                        "{} — Fission tutorials{page_label}",
                        category_title(&category.slug)
                    ),
                    Some(category_description(&category.slug).to_string()),
                    TutorialCategoryPage::new(category.clone(), page),
                )
                .with_route_structured_data(path, category_structured_data(category, page));
        }
    }

    let playground_apps = [
        "single-page-web-app-rust",
        "rust-product-inventory",
        "rust-admin-panel",
        "android-build-an-android-dashboard-with-fission",
    ];
    let playground_examples: Vec<PlaygroundExample> = playground_apps
        .iter()
        .filter_map(|app| {
            let goal = goals.iter().find(|goal| goal.meta.app == *app)?;
            let first_step = goal.steps.first()?;
            let source = first_step.files.get("src/app.rs")?.clone();
            Some(PlaygroundExample {
                app: goal.meta.app.clone(),
                title: goal.meta.title.clone(),
                description: goal.meta.description.clone(),
                source,
                preview_url: first_step.preview_url(&goal.meta.app),
            })
        })
        .collect();
    if let Some(first_example) = playground_examples.first() {
        site = site.route_widget::<DocsState, _>(
            navigation::TRY_IN_BROWSER,
            "Try Fission in your browser — interactive Rust app showcase",
            Some("Explore interactive Fission apps in your browser, switch between working examples and inspect the Rust component source behind each screen.".to_string()),
            PlaygroundPage::new(playground_examples.clone(), first_example.app.clone()),
        );

        for example in &playground_examples {
            let path = format!("{}{}/", navigation::TRY_IN_BROWSER, example.app);
            site = site.route_widget::<DocsState, _>(
                path,
                format!("{} — Try Fission in your browser", example.title),
                Some(example.description.clone()),
                PlaygroundPage::new(playground_examples.clone(), example.app.clone()),
            );
        }
    }

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

fn tutorials_structured_data(categories: &[crate::screens::TutorialCategory]) -> [String; 1] {
    [serde_json::json!({
        "@context": "https://schema.org",
        "@type": "CollectionPage",
        "name": "Fission tutorials: build Rust apps and websites",
        "description": "Step-by-step Fission tutorials for building complete Rust apps and websites.",
        "url": format!("https://fission.rs{}", navigation::TUTORIALS),
        "mainEntity": {
            "@type": "ItemList",
            "itemListElement": categories.iter().enumerate().map(|(index, category)| serde_json::json!({
                "@type": "ListItem",
                "position": index + 1,
                "name": category_title(&category.slug),
                "url": format!("https://fission.rs{}", category.route(1))
            })).collect::<Vec<_>>()
        }
    })
    .to_string()]
}

fn category_title(slug: &str) -> &'static str {
    match slug {
        "projects" => "Rust app and website projects",
        "mobile-setup" => "Set up Android and iOS development",
        "android" => "Android app tutorials",
        "ios" => "iOS app tutorials",
        "shared-mobile" => "Cross-platform mobile app tutorials",
        _ => "Fission tutorials",
    }
}

fn category_description(slug: &str) -> &'static str {
    match slug {
        "projects" => "Build complete Rust apps and websites with step-by-step Fission tutorials. Learn how to shape interfaces, manage state, handle routes and ship a working result.",
        "mobile-setup" => "Prepare a Rust development environment for Android or iOS, including the platform tools needed to build and run Fission apps.",
        "android" => "Build, test and release Android apps in Rust with Fission. These practical tutorials cover layouts, state, navigation, device features and Android packaging.",
        "ios" => "Build, test and release iOS apps in Rust with Fission. Follow practical tutorials for iPhone and iPad layouts, state, permissions, device features and TestFlight.",
        "shared-mobile" => "Build cross-platform mobile apps for Android and iOS with shared Rust components, offline workflows, device capabilities and responsive dashboards.",
        _ => "Step-by-step tutorials for building applications with Rust and Fission.",
    }
}

fn category_structured_data(
    category: &crate::screens::TutorialCategory,
    page: usize,
) -> [String; 1] {
    [serde_json::json!({
        "@context": "https://schema.org",
        "@type": "CollectionPage",
        "name": category_title(&category.slug),
        "description": category_description(&category.slug),
        "url": format!("https://fission.rs{}", category.route(page)),
        "mainEntity": {
            "@type": "ItemList",
            "itemListElement": category.tutorials
                .iter()
                .skip((page.saturating_sub(1)) * TUTORIALS_PER_PAGE)
                .take(TUTORIALS_PER_PAGE)
                .enumerate()
                .map(|(index, tutorial)| serde_json::json!({
                    "@type": "ListItem",
                    "position": (page - 1) * TUTORIALS_PER_PAGE + index + 1,
                    "name": tutorial.title,
                    "url": format!("https://fission.rs{}", tutorial.path)
                }))
                .collect::<Vec<_>>()
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
