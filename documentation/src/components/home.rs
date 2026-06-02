use super::home_nav::HomePageNav;
use super::home_sections::{
    ArchitectureSection, ChartsSection, ExamplesSection, FinalCta, HomePageHero, LifecycleSection,
    ModelSection, ProofStrip, TargetsSection,
};
use super::home_widgets::{content_width, page_fill};
use super::state::DocsState;
use fission::op::{AlignItems, JustifyContent};
use fission::prelude::*;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct RoutedHomePage {
    current_path: String,
}

impl RoutedHomePage {
    pub(crate) fn new(current_path: impl Into<String>) -> Self {
        Self {
            current_path: current_path.into(),
        }
    }
}

impl From<RoutedHomePage> for Widget {
    fn from(component: RoutedHomePage) -> Self {
        Router::<DocsState> {
            current_path: component.current_path.clone(),
            routes: vec![Route {
                path: "/".to_string(),
                builder: Arc::new(|_, _, _| HomePage.into()),
            }],
            not_found: None,
        }
        .into()
    }
}
#[derive(Clone, Debug)]
struct HomePage;

impl From<HomePage> for Widget {
    fn from(_component: HomePage) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                HomePageNav.into(),
                Row {
                    children: vec![Container::new(Column {
                        children: vec![
                            HomePageHero.into(),
                            ProofStrip.into(),
                            LifecycleSection.into(),
                            ArchitectureSection.into(),
                            ChartsSection.into(),
                            ModelSection.into(),
                            TargetsSection.into(),
                            ExamplesSection.into(),
                            FinalCta.into(),
                        ],
                        gap: Some(tokens.spacing.xxxl),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    })
                    .width(content_width(tokens))
                    .padding([0.0, 0.0, tokens.spacing.xxl, tokens.spacing.xxxxl])
                    .into()],
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(0.0),
            flex_grow: 1.0,
            ..Default::default()
        })
        .min_height(tokens.spacing.xxxxl * 9.0)
        .bg_fill(page_fill(tokens))
        .into()
    }
}
