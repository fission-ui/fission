use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub DocsHeader<DocsState> {
        props { hub: bool }
    }
    view(header, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let brand = if header.hub { "docs.header.brand" } else { "site.brand" };
    } =>
    SemanticsRegion(
        identifier: Some("docs-header".to_string()),
        label: Some(cx.view.env().tr("site.primary_nav")),
        child: Row(align_items: ir_op::AlignItems::Center, children: [
            SemanticsRegion(
                identifier: Some("docs-header-brand".to_string()),
                child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                    Image::asset(if header.hub { "/img/fission-mark-docs.svg" } else { "/img/fission-mark.svg" })
                        .size(28.0, 28.0)
                        .semantic_label(cx.view.env().tr("site.brand")),
                    copy(brand).weight(tokens.typography.font_weight_bold),
                ]),
            )
            .hyperlink(Hyperlink::new(navigation::HOME)),
            if header.hub {
                SemanticsRegion(
                    identifier: Some("docs-header-mobile-nav".to_string()),
                    label: Some(cx.view.env().tr("site.primary_nav")),
                    child: Row(gap: tokens.spacing.l, align_items: ir_op::AlignItems::Center, children: [
                        Link::to(cx.view.env().tr("site.docs"), navigation::DOCS),
                        Link::to(cx.view.env().tr("docs.header.components"), "/reference/widgets/catalog/"),
                        Link::to(cx.view.env().tr("site.tutorials"), navigation::TUTORIALS),
                        Link::to(cx.view.env().tr("site.examples"), navigation::EXAMPLES),
                        Link::to(cx.view.env().tr("docs.header.community"), navigation::COMMUNITY),
                    ]),
                )
            } else {
                SemanticsRegion(
                    identifier: Some("docs-header-nav".to_string()),
                    label: Some(cx.view.env().tr("site.primary_nav")),
                    child: Row(gap: tokens.spacing.l, align_items: ir_op::AlignItems::Center, children: [
                        Link::to(cx.view.env().tr("site.docs"), navigation::DOCS),
                        Link::to(cx.view.env().tr("docs.header.components"), "/reference/widgets/catalog/"),
                        Link::to(cx.view.env().tr("site.tutorials"), navigation::TUTORIALS),
                        Link::to(cx.view.env().tr("site.examples"), navigation::EXAMPLES),
                        Link::to(cx.view.env().tr("docs.header.community"), navigation::COMMUNITY),
                    ]),
                )
            },
            Spacer(flex_grow: 1.0),
            SemanticsRegion(
                identifier: Some("site-search-trigger".to_string()),
                label: Some(cx.view.env().tr("site.search")),
                child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                    Icon::svg(material::action::search::regular()).size(tokens.spacing.m),
                    copy(if header.hub { "docs.header.search" } else { "site.search" }),
                    Spacer(flex_grow: 1.0),
                    Kbd(text: "⌘ K".to_string()),
                ]),
            ),
            Spacer(flex_grow: 1.0),
            SemanticsRegion(
                identifier: Some("docs-header-github".to_string()),
                child: Some(Link::to(cx.view.env().tr("site.github"), navigation::GITHUB).into()),
            ),
            SemanticsRegion(
                identifier: Some("site-theme-toggle".to_string()),
                label: Some(cx.view.env().tr("docs.header.theme")),
                child: Some(Icon::svg(material::device::dark_mode::regular())
                    .size(tokens.spacing.m)
                    .color(tokens.colors.text_primary)
                    .into()),
            ),
        ]),
    )
}
