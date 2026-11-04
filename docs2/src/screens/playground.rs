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
                                        identifier: Some(format!("playground-example-{}", example.app)),
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
                        child: Row(
                            gap: tokens.spacing.xl,
                            align_items: ir_op::AlignItems::Stretch,
                            children: [
                                SemanticsRegion(
                                    identifier: Some("playground-source".to_string()),
                                    label: Some(cx.view.env().tr("playground.source")),
                                    child: Column(gap: tokens.spacing.m, children: [
                                        SemanticsRegion(
                                            copy("playground.source").weight(tokens.typography.font_weight_bold),
                                            identifier: Some("site-heading-2:playground-source".to_string()),
                                        ),
                                        Text(selected.description.clone()).color(tokens.colors.text_secondary),
                                        MarkdownContent::new(source),
                                    ]),
                                ),
                                SemanticsRegion(
                                    identifier: Some("playground-preview".to_string()),
                                    label: Some(cx.view.env().tr("playground.preview")),
                                    child: Column(gap: tokens.spacing.s, children: [
                                        SemanticsRegion(
                                            copy("playground.preview").weight(tokens.typography.font_weight_bold),
                                            identifier: Some("site-heading-2:playground-preview".to_string()),
                                        ),
                                        WebView(
                                            id: preview_id,
                                            url: selected.preview_url.clone(),
                                            label: Some(selected.title.clone()),
                                            height: Some(620.0),
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
