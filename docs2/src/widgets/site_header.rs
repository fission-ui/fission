use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub SiteHeader<DocsState> {}
    view(_header, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("site-header".to_string()),
        label: Some(cx.view.env().tr("site.primary_nav")),
        child: Row(
            gap: tokens.spacing.l,
            align_items: ir_op::AlignItems::Center,
            children: [
                SemanticsRegion(
                    identifier: Some("docs-brand".to_string()),
                    child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                        Image::asset("/img/fission-mark.svg")
                            .size(28.0, 28.0)
                            .semantic_label(cx.view.env().tr("site.brand")),
                        copy("site.brand").weight(tokens.typography.font_weight_bold),
                    ]),
                )
                .hyperlink(Hyperlink::new("/")),
                Row(
                    gap: tokens.spacing.l,
                    children: [
                        Link::to(cx.view.env().tr("site.docs"), navigation::DOCS),
                        Link::to(cx.view.env().tr("site.tutorials"), navigation::COUNTER_TUTORIAL),
                        Link::to(cx.view.env().tr("site.compose"), navigation::COMPOSE),
                        Link::to(cx.view.env().tr("site.examples"), navigation::SHOWCASE),
                        Link::to(cx.view.env().tr("site.reference"), navigation::REFERENCE_OVERVIEW),
                    ],
                ),
                SemanticsRegion(
                    Link::to(cx.view.env().tr("site.docs"), navigation::DOCS),
                    identifier: Some("mobile-docs-link".to_string()),
                ),
                Spacer(flex_grow: 1.0),
                SemanticsRegion(
                    identifier: Some("docs-search".to_string()),
                    label: Some(cx.view.env().tr("site.search")),
                    child: Row(gap: tokens.spacing.s, children: [
                        Icon::svg(material::action::search::regular()).size(tokens.spacing.m),
                        copy("site.search"),
                        Kbd(text: "/".to_string()),
                    ]),
                ),
                Link::to(cx.view.env().tr("site.github"), navigation::GITHUB),
            ],
        ),
    )
}
