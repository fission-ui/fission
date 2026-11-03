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
            wrap: ir_op::FlexWrap::Wrap,
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
                Spacer(flex_grow: 1.0),
                SemanticsRegion(
                    identifier: Some("site-primary-nav".to_string()),
                    label: Some(cx.view.env().tr("site.primary_nav")),
                    child: Row(
                        gap: tokens.spacing.l,
                        wrap: ir_op::FlexWrap::Wrap,
                        children: [
                            Link::to(cx.view.env().tr("site.docs"), navigation::DOCS),
                            Link::to(cx.view.env().tr("site.tutorials"), navigation::TUTORIALS),
                            Link::to(cx.view.env().tr("site.compose"), navigation::COMPOSE),
                            Link::to(cx.view.env().tr("site.examples"), navigation::EXAMPLES),
                            Link::to(cx.view.env().tr("site.reference"), navigation::REFERENCE_OVERVIEW),
                        ],
                    ),
                ),
                SemanticsRegion(
                    identifier: Some("site-sidebar-toggle".to_string()),
                    label: Some(cx.view.env().tr("site.primary_nav")),
                    child: Some(
                        copy("site.menu")
                            .weight(tokens.typography.font_weight_semibold)
                            .into(),
                    ),
                ),
                SemanticsRegion(
                    identifier: Some("site-header-cta".to_string()),
                    child: Some(Link::to(
                        cx.view.env().tr("site.header_try"),
                        navigation::TRY_IN_BROWSER,
                    ).into()),
                ),
                SemanticsRegion(
                    identifier: Some("site-search-trigger".to_string()),
                    label: Some(cx.view.env().tr("site.search")),
                    child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                        Icon::svg(material::action::search::regular()).size(tokens.spacing.m),
                        copy("site.search"),
                        Kbd(text: "/".to_string()),
                    ]),
                ),
                SemanticsRegion(
                    identifier: Some("site-github-link".to_string()),
                    child: Some(Link::to(cx.view.env().tr("site.github"), navigation::GITHUB).into()),
                ),
                SemanticsRegion(
                    identifier: Some("site-mobile-menu".to_string()),
                    label: Some(cx.view.env().tr("site.primary_nav")),
                    child: Column(gap: tokens.spacing.xs, children: [
                        Link::to(cx.view.env().tr("site.docs"), navigation::DOCS),
                        Link::to(cx.view.env().tr("site.tutorials"), navigation::TUTORIALS),
                        Link::to(cx.view.env().tr("site.compose"), navigation::COMPOSE),
                        Link::to(cx.view.env().tr("site.examples"), navigation::EXAMPLES),
                        Link::to(cx.view.env().tr("site.reference"), navigation::REFERENCE_OVERVIEW),
                        Link::to(cx.view.env().tr("site.header_try"), navigation::TRY_IN_BROWSER),
                        Link::to(cx.view.env().tr("site.github"), navigation::GITHUB),
                    ]),
                ),
            ],
        ),
    )
}
