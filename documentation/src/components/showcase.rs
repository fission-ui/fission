use super::home_nav::HomePageNav;
use super::home_widgets::site_semantics;
use super::state::DocsState;
use fission::op::Fill;
use fission::prelude::*;

const SHOWCASE_LABEL: &str =
    "Interactive Fission Rust GUI and WebAssembly example application showcase";

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShowcasePage;

impl From<ShowcasePage> for Widget {
    fn from(_page: ShowcasePage) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;

        let showcase = Column {
            children: vec![WebView {
                id: WidgetId::explicit("documentation.showcase.web-app"),
                url: "/showcase-app/".into(),
                user_agent: None,
                label: Some(SHOWCASE_LABEL.into()),
                width: None,
                height: None,
            }
            .into()],
            semantics: Some(site_semantics("site-showcase-frame")),
            ..Default::default()
        };

        Container::new(Column {
            children: vec![HomePageNav.into(), showcase.into()],
            gap: Some(0.0),
            semantics: Some(site_semantics("site-showcase-page")),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .bg_fill(Fill::Solid(tokens.colors.background))
        .into()
    }
}
