use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

const BROWSER_CODE: &str = r#"fission_component! {
    pub App<()> {}
    view(_app, _state, _cx) =>
        Center(Text("Hello, Fission!"))
}"#;

fission_component! {
    pub BrowserCta<DocsState> {}
    view(_cta, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let highlighted_example = format!("```rust\n{}\n```", BROWSER_CODE);
    } =>
    SemanticsRegion(
        identifier: Some("home-browser".to_string()),
        label: Some(cx.view.env().tr("home.browser.title")),
        child: Row(
            gap: tokens.spacing.xl,
            align_items: ir_op::AlignItems::Center,
            children: [
                Column(gap: tokens.spacing.m, children: [
                    copy("home.browser.eyebrow")
                        .color(tokens.colors.text_secondary)
                        .weight(tokens.typography.font_weight_semibold),
                    SemanticsRegion(
                        copy("home.browser.title")
                            .weight(tokens.typography.font_weight_bold),
                        identifier: Some("site-heading-2:browser-playground".to_string()),
                    ),
                    copy("home.browser.summary").color(tokens.colors.text_secondary),
                    SemanticsRegion(
                        Link::to(
                            cx.view.env().tr("site.try_browser"),
                            navigation::TRY_IN_BROWSER,
                        ),
                        identifier: Some("home-browser-cta".to_string()),
                    ),
                ]),
                SemanticsRegion(
                    identifier: Some("home-browser-demo".to_string()),
                    child: Row(children: [
                        Column(children: [
                            copy("home.filename").weight(tokens.typography.font_weight_semibold),
                            MarkdownContent::new(highlighted_example),
                        ]),
                        WebView(
                            id: WidgetId::explicit("home-browser-counter"),
                            url: "/tutorials/single-page-web-app-rust/start-screen/?fission_renderer=canvas2d".to_string(),
                            label: Some(cx.view.env().tr("home.browser_demo_label")),
                            height: Some(400.0),
                        ),
                    ]),
                ),
            ],
        ),
    )
}
