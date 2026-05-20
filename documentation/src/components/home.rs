use super::home_nav::HomePageNav;
use super::home_sections::{
    ArchitectureSection, ChartsSection, ExamplesSection, FinalCta, HomePageHero, ModelSection,
    ProofStrip, TargetsSection,
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

impl Widget<DocsState> for RoutedHomePage {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        Router {
            current_path: self.current_path.clone(),
            routes: vec![Route {
                path: "/".to_string(),
                builder: Arc::new(|ctx, view, _params| HomePage.build(ctx, view)),
            }],
            not_found: None,
        }
        .build(ctx, view)
    }
}

#[derive(Clone, Debug)]
struct HomePage;

impl Widget<DocsState> for HomePage {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Column {
                children: vec![
                    HomePageNav.build(ctx, view),
                    Row {
                        children: vec![Container::new(
                            Column {
                                children: vec![
                                    HomePageHero.build(ctx, view),
                                    ProofStrip.build(ctx, view),
                                    ArchitectureSection.build(ctx, view),
                                    ChartsSection.build(ctx, view),
                                    ModelSection.build(ctx, view),
                                    TargetsSection.build(ctx, view),
                                    ExamplesSection.build(ctx, view),
                                    FinalCta.build(ctx, view),
                                ],
                                gap: Some(tokens.spacing.xxxl),
                                align_items: AlignItems::Center,
                                ..Default::default()
                            }
                            .into_node(),
                        )
                        .width(content_width(tokens))
                        .padding([0.0, 0.0, tokens.spacing.xxl, tokens.spacing.xxxxl])
                        .into_node()],
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                gap: Some(0.0),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
        )
        .min_height(tokens.spacing.xxxxl * 9.0)
        .bg_fill(page_fill(tokens))
        .into_node()
    }
}
