use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, SiteFooter, SiteHeader};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct PlaygroundExample {
    pub app: String,
    pub title: String,
    pub description: String,
    pub source: String,
    pub preview_url: String,
    pub preview_height: f32,
}

fission_component! {
    pub PlaygroundPage<DocsState> {
        props {
            examples: Vec<PlaygroundExample>,
            selected_app: String,
        }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let selected = page.examples.iter().find(|example| example.app == page.selected_app)
            .unwrap_or_else(|| page.examples.first().expect("playground needs at least one example"));
        let source = format!("```rust\n{}\n```", selected.source);
        let preview_id = WidgetId::explicit(&format!("browser-showcase-{}", selected.app));
    } =>
    Container(
        Column(gap: tokens.spacing.xl, children: [
            SiteHeader(),
            SemanticsRegion(
                identifier: Some("playground-main".to_string()),
                child: Column(gap: tokens.spacing.xl, children: [
                    Column(gap: tokens.spacing.m, children: [
                        SemanticsRegion(
                            copy("playground.title").weight(tokens.typography.font_weight_bold),
                            identifier: Some("site-heading-1:playground".to_string()),
                        ),
                        copy("playground.description").color(tokens.colors.text_secondary),
                    ]),
                    SemanticsRegion(
                        identifier: Some("playground-examples".to_string()),
                        label: Some(cx.view.env().tr("playground.examples")),
                        child: Row(
                            gap: tokens.spacing.s,
                            wrap: ir_op::FlexWrap::Wrap,
                            children: [
                                for example in &page.examples {
                                    SemanticsRegion(
                                        identifier: Some(if example.app == selected.app {
                                            "playground-example-active".to_string()
                                        } else {
                                            "playground-example".to_string()
                                        }),
                                        label: Some(example.title.clone()),
                                        child: Some(
                                            Link::to(
                                                example.title.clone(),
                                                format!("{}{}/", navigation::TRY_IN_BROWSER, example.app),
                                            )
                                            .into(),
                                        ),
                                        key: example.app.clone(),
                                    )
                                },
                            ],
                        ),
                    ),
                    SemanticsRegion(
                        identifier: Some("playground-workspace".to_string()),
                        child: Column(
                            gap: tokens.spacing.xl,
                            children: [
                                SemanticsRegion(
                                    identifier: Some("playground-preview".to_string()),
                                    label: Some(cx.view.env().tr("playground.preview")),
                                    child: Column(gap: tokens.spacing.m, children: [
                                        SemanticsRegion(
                                            Text(selected.title.clone())
                                                .weight(tokens.typography.font_weight_bold),
                                            identifier: Some("site-heading-2:playground-preview".to_string()),
                                        ),
                                        Text(selected.description.clone()).color(tokens.colors.text_secondary),
                                        WebView(
                                            id: preview_id,
                                            url: selected.preview_url.clone(),
                                            label: Some(selected.title.clone()),
                                            height: Some(selected.preview_height),
                                        ),
                                    ]),
                                ),
                                SemanticsRegion(
                                    identifier: Some("site-disclosure:playground-source".to_string()),
                                    child: Column(gap: tokens.spacing.m, children: [
                                        SemanticsRegion(
                                            identifier: Some("site-disclosure-summary:playground-source".to_string()),
                                            child: Some(copy("playground.source.show").into()),
                                        ),
                                        SemanticsRegion(
                                            identifier: Some("playground-source".to_string()),
                                            label: Some(cx.view.env().tr("playground.source")),
                                            child: Column(gap: tokens.spacing.m, children: [
                                                SemanticsRegion(
                                                    copy("playground.source").weight(tokens.typography.font_weight_bold),
                                                    identifier: Some("site-heading-2:playground-source".to_string()),
                                                ),
                                                MarkdownContent::new(source),
                                            ]),
                                        ),
                                    ]),
                                ),
                            ],
                        ),
                    ),
                    Link::to(cx.view.env().tr("playground.tutorials"), navigation::TUTORIALS),
                ]),
            ),
            SiteFooter(),
        ]),
    )
    .width_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}
