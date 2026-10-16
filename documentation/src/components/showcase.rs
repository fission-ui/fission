use super::home_nav::HomePageNav;
use super::home_widgets::{site_semantics, SemanticColumn};
use super::landing::landing_width;
use super::state::DocsState;
use fission::op::{AlignItems, Fill};
use fission::prelude::*;

const SHOWCASE_LABEL: &str =
    "Interactive Fission Rust GUI and WebAssembly example application showcase";

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShowcasePage;

impl From<ShowcasePage> for Widget {
    fn from(_page: ShowcasePage) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;

        let introduction = SemanticColumn::new(
            "site-showcase-introduction",
            vec![
                Text::new("Fission Rust GUI and WASM example apps")
                    .size(tokens.typography.display_small_size)
                    .line_height(
                        tokens.typography.display_small_size * tokens.typography.line_height_tight,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .semantics_identifier("site-heading-1:top")
                    .into(),
                Text::new(
                    "Use the live showcase below to explore complete Fission interfaces, switch \
                     design systems and try the Inbox example's navigation, forms, menus and \
                     responsive layouts directly in your browser. Every preview is delivered as \
                     a static asset and runs without a Fission application server.",
                )
                .size(tokens.typography.body_large_size)
                .line_height(
                    tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                )
                .color(tokens.colors.text_secondary)
                .max_width(780.0)
                .into(),
            ],
            Some(tokens.spacing.m),
            AlignItems::Start,
        );

        let showcase = Column {
            children: vec![WebView {
                id: WidgetId::explicit("documentation.showcase.web-app"),
                url: "/showcase-app/".into(),
                user_agent: None,
                label: Some(SHOWCASE_LABEL.into()),
                width: None,
                height: Some(900.0),
            }
            .into()],
            semantics: Some(site_semantics("site-showcase-frame")),
            ..Default::default()
        };

        Container::new(Column {
            children: vec![
                HomePageNav.into(),
                Container::new(Column {
                    children: vec![introduction.into(), showcase.into()],
                    gap: Some(tokens.spacing.xl),
                    ..Default::default()
                })
                .width_length(Length::percent(100.0))
                .max_width(landing_width(tokens))
                .padding([
                    tokens.spacing.xl,
                    tokens.spacing.xl,
                    tokens.spacing.xxl,
                    tokens.spacing.xl,
                ])
                .into(),
            ],
            gap: Some(0.0),
            align_items: AlignItems::Center,
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .bg_fill(Fill::Solid(tokens.colors.background))
        .into()
    }
}
