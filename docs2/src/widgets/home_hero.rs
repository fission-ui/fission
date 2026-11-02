use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

const HERO_CODE: &str = r#"fission_component! {
    pub Counter<()> {
        state { count: i32 = 0 }
    }
    view(counter, _state, _cx) =>
        Button(
            child: Text(format!(
                "Clicked {} times",
                counter.count
            )),
            on_press: reduce {
                counter.count += 1;
            },
        )
}"#;

fission_component! {
    pub HomeHero<DocsState> {}
    view(_hero, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
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
                        copy("home.title")
                            .size(tokens.typography.display_md_size)
                            .weight(tokens.typography.font_weight_bold),
                        copy("home.summary")
                            .size(tokens.typography.body_large_size)
                            .color(tokens.colors.text_secondary),
                        Row(gap: tokens.spacing.m, children: [
                            SemanticsRegion(
                                Link::to(
                                    cx.view.env().tr("home.get_started"),
                                    navigation::COUNTER_TUTORIAL,
                                ),
                                identifier: Some("home-primary-cta".to_string()),
                            ),
                            SemanticsRegion(
                                Link::to(
                                    cx.view.env().tr("home.see_action"),
                                    navigation::SHOWCASE,
                                ),
                                identifier: Some("home-secondary-cta".to_string()),
                            ),
                        ]),
                    ],
                ),
                SemanticsRegion(
                    identifier: Some("home-hero-demo".to_string()),
                    label: Some(cx.view.env().tr("home.demo_label")),
                    child: Row(
                        children: [
                            SemanticsRegion(
                                identifier: Some("home-hero-code".to_string()),
                                child: Column(children: [
                                    copy("home.filename").weight(tokens.typography.font_weight_semibold),
                                    Code(text: HERO_CODE.to_string()),
                                ]),
                            ),
                            SemanticsRegion(
                                identifier: Some("home-hero-preview".to_string()),
                                child: Column(children: [
                                    Row(children: [copy("site.brand"), Spacer(flex_grow: 1.0), Text("•••")]),
                                    WebView(
                                        id: WidgetId::explicit("home-live-counter"),
                                        url: "/tutorials/interactive-counter/increment/?fission_renderer=canvas2d".to_string(),
                                        label: Some(cx.view.env().tr("home.counter_label")),
                                        height: Some(400.0),
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
