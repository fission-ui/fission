use super::home_nav::HomePageNav;
use super::landing::LandingHero;
use super::landing_sections::{CtaBand, DeliverySection, FoundationSection, TargetsSection};
use super::state::DocsState;
use fission::op::{AlignItems, Fill, JustifyContent};
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
                            LandingHero.into(),
                            FoundationSection.into(),
                            TargetsSection.into(),
                            DeliverySection.into(),
                            CtaBand.into(),
                        ],
                        gap: Some(0.0),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    })
                    .width_length(Length::percent(100.0))
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
        .bg_fill(Fill::Solid(tokens.colors.background))
        .into()
    }
}
