use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub HomeHero<DocsState> {
        props {
            source: String,
            preview_url: String,
        }
    }
    view(_hero, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let highlighted_source = format!("```rust\n{}\n```", _hero.source);
        let excerpt = _hero.source.lines()
            .skip_while(|line| !line.contains("fission_component!"))
            .take(12)
            .collect::<Vec<_>>()
            .join("\n");
        let highlighted_excerpt = format!("```rust\n{excerpt}\n```");
    } =>
    SemanticsRegion(
        identifier: Some("home-hero".to_string()),
        label: Some(cx.view.env().tr("home.title")),
        child: Row(
            gap: tokens.spacing.xxl,
            align_items: ir_op::AlignItems::Center,
            children: [
                Column(
                    gap: tokens.spacing.l,
                    children: [
                        copy("home.eyebrow")
                            .color(tokens.colors.primary)
                            .weight(tokens.typography.font_weight_semibold),
                        SemanticsRegion(
                            copy("home.title")
                                .family(tokens.typography.font_family_serif.clone())
                                .size(tokens.typography.display_sm_size)
                                .line_height(tokens.typography.display_sm_size * tokens.typography.line_height_heading)
                                .weight(tokens.typography.font_weight_bold),
                            identifier: Some("site-heading-1:home".to_string()),
                        ),
                        copy("home.summary")
                            .color(tokens.colors.text_secondary),
                        Row(gap: tokens.spacing.m, children: [
                            SemanticsRegion(
                                Link::to(
                                    cx.view.env().tr("home.get_started"),
                                    navigation::QUICKSTART,
                                ),
                                identifier: Some("home-primary-cta".to_string()),
                            ),
                            SemanticsRegion(
                                Link::to(
                                    cx.view.env().tr("home.see_action"),
                                    navigation::TRY_IN_BROWSER,
                                ),
                                identifier: Some("home-secondary-cta".to_string()),
                            ),
                        ]),
                    ],
                ),
                SemanticsRegion(
                    identifier: Some("home-hero-demo".to_string()),
                    label: Some(cx.view.env().tr("home.demo_label")),
                    child: Column(
                        children: [
                            SemanticsRegion(
                                identifier: Some("home-hero-stage".to_string()),
                                child: Row(children: [
                                    SemanticsRegion(
                                        identifier: Some("home-hero-code".to_string()),
                                        child: Column(children: [
                                            Text("app.rs").weight(tokens.typography.font_weight_semibold),
                                            MarkdownContent::new(highlighted_excerpt),
                                        ]),
                                    ),
                                    SemanticsRegion(
                                        identifier: Some("home-hero-preview".to_string()),
                                        child: Column(children: [
                                            Row(children: [
                                                copy("site.brand"),
                                                Spacer(flex_grow: 1.0),
                                                Link::to(cx.view.env().tr("home.see_action"), _hero.preview_url.clone()),
                                            ]),
                                            WebView(
                                                id: WidgetId::explicit("home-live-task-workspace"),
                                                url: _hero.preview_url.clone(),
                                                label: Some(cx.view.env().tr("home.demo_label")),
                                                height: Some(312.0),
                                            ),
                                        ]),
                                    ),
                                ]),
                            ),
                            SemanticsRegion(
                                identifier: Some("site-disclosure:home-source".to_string()),
                                child: Column(children: [
                                    SemanticsRegion(
                                        identifier: Some("site-disclosure-summary:home-source".to_string()),
                                        child: Some(copy("home.source").into()),
                                    ),
                                    SemanticsRegion(
                                        identifier: Some("home-hero-full-source".to_string()),
                                        child: Column(children: [
                                            Text("app.rs").weight(tokens.typography.font_weight_semibold),
                                            MarkdownContent::new(highlighted_source),
                                        ]),
                                    ),
                                ]),
                            ),
                        ],
                    ),
                ),
            ],
        ),
    )
}
