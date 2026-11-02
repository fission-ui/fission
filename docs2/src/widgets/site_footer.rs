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
        child: Row(
            gap: tokens.spacing.l,
            wrap: ir_op::FlexWrap::Wrap,
            align_items: ir_op::AlignItems::Center,
            children: [
                Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                    Image::asset("/img/fission-mark.svg").size(24.0, 24.0).semantic_label(""),
                    copy("site.brand").weight(tokens.typography.font_weight_bold),
                ]),
                copy("site.footer_tagline").color(tokens.colors.text_secondary),
                Spacer(flex_grow: 1.0),
                Link::to(cx.view.env().tr("site.docs"), navigation::DOCS),
                Link::to(cx.view.env().tr("site.compose"), navigation::COMPOSE),
                Link::to(cx.view.env().tr("site.reference"), navigation::REFERENCE_OVERVIEW),
                Link::to(cx.view.env().tr("site.try_browser"), navigation::SHOWCASE),
            ],
        ),
    )
}
