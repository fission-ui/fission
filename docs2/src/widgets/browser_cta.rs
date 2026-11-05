use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub PlaygroundCallout<DocsState> {}
    view(_callout, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-browser".to_string()),
        label: Some(cx.view.env().tr("home.browser.title")),
        child: Row(
            gap: tokens.spacing.xxl,
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
            ],
        ),
    )
}
