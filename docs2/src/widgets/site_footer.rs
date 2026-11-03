use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub SiteFooter<DocsState> {}
    view(_footer, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("site-footer".to_string()),
        label: Some(cx.view.env().tr("site.footer_label")),
        child: Column(gap: tokens.spacing.m, children: [
            Row(
                gap: tokens.spacing.l,
                wrap: ir_op::FlexWrap::Wrap,
                align_items: ir_op::AlignItems::Center,
                children: [
                    Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                        Image::asset("/img/fission-mark.svg").size(24.0, 24.0).semantic_label(""),
                        copy("site.brand").weight(tokens.typography.font_weight_bold),
                    ]),
                    copy("site.copyright").color(tokens.colors.text_secondary),
                    Spacer(flex_grow: 1.0),
                    Link::to(cx.view.env().tr("site.github"), navigation::GITHUB),
                ],
            ),
            SemanticsRegion(
                identifier: Some("site-tutorial-links".to_string()),
                label: Some(cx.view.env().tr("site.footer_tutorial_links")),
                child: Row(
                    gap: tokens.spacing.m,
                    wrap: ir_op::FlexWrap::Wrap,
                    align_items: ir_op::AlignItems::Center,
                    children: [
                        copy("site.footer_tutorial_links").weight(tokens.typography.font_weight_bold),
                        Link::to(cx.view.env().tr("site.tutorials"), navigation::TUTORIALS),
                        Link::to(
                            cx.view.env().tr("site.footer_tutorial_web"),
                            "/blog/build-a-rust-website-with-dropdown-navigation/",
                        ),
                        Link::to(
                            cx.view.env().tr("site.footer_tutorial_login"),
                            "/blog/build-a-login-page-in-rust/",
                        ),
                        Link::to(
                            cx.view.env().tr("site.footer_tutorial_admin"),
                            "/blog/build-an-admin-panel-in-rust/",
                        ),
                    ],
                ),
            ),
        ]),
    )
}
