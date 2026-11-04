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
        child: Column(gap: tokens.spacing.xl, children: [
            Row(
                gap: tokens.spacing.xxl,
                wrap: ir_op::FlexWrap::Wrap,
                align_items: ir_op::AlignItems::Start,
                children: [
                    Column(gap: tokens.spacing.s, children: [
                        Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                            Image::asset("/img/fission-mark.svg").size(24.0, 24.0).semantic_label(""),
                            copy("site.brand").weight(tokens.typography.font_weight_bold),
                        ]),
                        copy("site.footer_tagline").color(tokens.colors.text_secondary),
                    ]),
                    Row(
                        gap: tokens.spacing.xl,
                        wrap: ir_op::FlexWrap::Wrap,
                        align_items: ir_op::AlignItems::Start,
                        children: [
                            SemanticsRegion(
                                identifier: Some("site-footer-product".to_string()),
                                label: Some(cx.view.env().tr("site.footer.product")),
                                child: Column(gap: tokens.spacing.s, children: [
                                    copy("site.footer.product").weight(tokens.typography.font_weight_bold),
                                    Link::to(cx.view.env().tr("site.footer.product_overview"), navigation::PRODUCTS),
                                    Link::to(cx.view.env().tr("site.footer.cross_platform"), navigation::PRODUCT_CROSS_PLATFORM),
                                    Link::to(cx.view.env().tr("site.footer.static_sites"), navigation::PRODUCT_STATIC_SITES),
                                    Link::to(cx.view.env().tr("site.footer.server_sites"), navigation::PRODUCT_SERVER_SITES),
                                    Link::to(cx.view.env().tr("site.footer.product_charts"), navigation::PRODUCT_CHARTS),
                                ]),
                            ),
                            SemanticsRegion(
                                identifier: Some("site-footer-start".to_string()),
                                label: Some(cx.view.env().tr("site.footer.start_building")),
                                child: Column(gap: tokens.spacing.s, children: [
                                    copy("site.footer.start_building").weight(tokens.typography.font_weight_bold),
                                    Link::to(cx.view.env().tr("site.footer.quickstart"), navigation::QUICKSTART),
                                    Link::to(cx.view.env().tr("site.docs"), navigation::DOCS),
                                    Link::to(cx.view.env().tr("site.compose"), navigation::COMPOSE),
                                    Link::to(cx.view.env().tr("site.try_browser"), navigation::TRY_IN_BROWSER),
                                ]),
                            ),
                            SemanticsRegion(
                                identifier: Some("site-footer-tutorials".to_string()),
                                label: Some(cx.view.env().tr("site.footer.tutorials")),
                                child: Column(gap: tokens.spacing.s, children: [
                                    copy("site.footer.tutorials").weight(tokens.typography.font_weight_bold),
                                    Link::to(cx.view.env().tr("site.footer.all_tutorials"), navigation::TUTORIALS),
                                    Link::to(cx.view.env().tr("site.footer.tutorial_website"), navigation::WEBSITE_NAVIGATION),
                                    Link::to(cx.view.env().tr("site.footer.tutorial_login"), navigation::LOGIN_PAGE),
                                    Link::to(cx.view.env().tr("site.footer.tutorial_mobile"), navigation::MOBILE_APPS),
                                    Link::to(cx.view.env().tr("site.footer.tutorial_crud"), navigation::CRUD_APP),
                                ]),
                            ),
                            SemanticsRegion(
                                identifier: Some("site-footer-reference".to_string()),
                                label: Some(cx.view.env().tr("site.footer.reference")),
                                child: Column(gap: tokens.spacing.s, children: [
                                    copy("site.footer.reference").weight(tokens.typography.font_weight_bold),
                                    Link::to(cx.view.env().tr("site.reference"), navigation::REFERENCE_OVERVIEW),
                                    Link::to(cx.view.env().tr("site.footer.widget_catalog"), navigation::WIDGET_CATALOG),
                                    Link::to(cx.view.env().tr("site.footer.state_guide"), navigation::STATE_GUIDE),
                                    Link::to(cx.view.env().tr("site.footer.charts"), navigation::CHARTS_REFERENCE),
                                    Link::to(cx.view.env().tr("site.footer.accessibility"), navigation::ACCESSIBILITY_GUIDE),
                                ]),
                            ),
                            SemanticsRegion(
                                identifier: Some("site-footer-community".to_string()),
                                label: Some(cx.view.env().tr("site.footer.community")),
                                child: Column(gap: tokens.spacing.s, children: [
                                    copy("site.footer.community").weight(tokens.typography.font_weight_bold),
                                    Link::to(cx.view.env().tr("site.examples"), navigation::EXAMPLES),
                                    Link::to(cx.view.env().tr("site.github"), navigation::GITHUB),
                                    Link::to(cx.view.env().tr("site.footer.discussions"), navigation::COMMUNITY),
                                    Link::to(cx.view.env().tr("site.footer.releases"), navigation::CHANGELOG),
                                    Link::to(cx.view.env().tr("site.footer.contributing"), navigation::CONTRIBUTING),
                                ]),
                            ),
                        ],
                    ),
                ],
            ),
            Row(
                wrap: ir_op::FlexWrap::Wrap,
                align_items: ir_op::AlignItems::Center,
                children: [
                    copy("site.copyright").color(tokens.colors.text_secondary),
                    Spacer(flex_grow: 1.0),
                ],
            ),
        ]),
    )
}
