use crate::navigation;
use crate::products::ProductRoute;
use crate::state::DocsState;
use crate::widgets::{copy, ProductBenefit, ProductLinkCard, SiteFooter, SiteHeader};
use fission::prelude::*;

fission_component! {
    pub ProductPage<DocsState> {
        props {
            product: ProductRoute,
            products: Vec<ProductRoute>,
        }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let product_items: Vec<_> = page
            .products
            .iter()
            .filter(|item| item.slug != "overview")
            .collect();
        let title_heading_identifier = format!(
            "site-heading-1:product-{}",
            page.product.slug
        );
    } =>
    Container(
        Column(gap: tokens.spacing.xl, children: [
            SiteHeader(),
            SemanticsRegion(
                identifier: Some("product-main".to_string()),
                label: Some(cx.view.env().tr(page.product.title_key)),
                child: Column(gap: tokens.spacing.xxl, children: [
                    SemanticsRegion(
                        identifier: Some("product-hero".to_string()),
                        child: Row(
                            gap: tokens.spacing.xl,
                            wrap: ir_op::FlexWrap::Wrap,
                            align_items: ir_op::AlignItems::Center,
                            children: [
                                Column(gap: tokens.spacing.l, children: [
                                    copy(page.product.eyebrow_key)
                                        .color(tokens.colors.primary)
                                        .weight(tokens.typography.font_weight_semibold),
                                    SemanticsRegion(
                                        copy(page.product.title_key)
                                            .size(tokens.typography.display_md_size)
                                            .weight(tokens.typography.font_weight_bold)
                                            .wrap(true),
                                        identifier: Some(title_heading_identifier),
                                    ),
                                    copy(page.product.summary_key)
                                        .size(tokens.typography.body_large_size)
                                        .color(tokens.colors.text_secondary)
                                        .wrap(true),
                                    Row(gap: tokens.spacing.m, wrap: ir_op::FlexWrap::Wrap, children: [
                                        Link::to(
                                            cx.view.env().tr("product.get_started"),
                                            page.product.tutorial_path,
                                        ),
                                        Link::to(
                                            cx.view.env().tr("product.reference"),
                                            page.product.reference_path,
                                        ),
                                    ]),
                                ]),
                                Image::asset(page.product.image)
                                    .size(280.0, 210.0)
                                    .fit(ir_op::ImageFit::Cover)
                                    .semantic_label(cx.view.env().tr(page.product.image_alt_key)),
                            ],
                        ),
                    ),
                    SemanticsRegion(
                        identifier: Some("product-benefits".to_string()),
                        child: Column(gap: tokens.spacing.l, children: [
                            SemanticsRegion(
                                copy("product.benefits.title")
                                    .size(tokens.typography.heading1_size)
                                    .weight(tokens.typography.font_weight_bold),
                                identifier: Some("site-heading-2:product-benefits".to_string()),
                            ),
                            Row(gap: tokens.spacing.l, wrap: ir_op::FlexWrap::Wrap, children: [
                                for benefit_key in &page.product.benefits {
                                    ProductBenefit(*benefit_key, key: (*benefit_key).to_string())
                                },
                            ]),
                        ]),
                    ),
                    if page.product.slug == "overview" {
                        SemanticsRegion(
                            identifier: Some("product-catalog".to_string()),
                            child: Column(gap: tokens.spacing.l, children: [
                                SemanticsRegion(
                                    copy("product.catalog.title")
                                        .size(tokens.typography.heading1_size)
                                        .weight(tokens.typography.font_weight_bold),
                                    identifier: Some("site-heading-2:product-catalog".to_string()),
                                ),
                                Row(gap: tokens.spacing.m, wrap: ir_op::FlexWrap::Wrap, children: [
                                    for item in &product_items {
                                        ProductLinkCard(
                                            item.title_key,
                                            item.summary_key,
                                            item.path,
                                            key: item.slug,
                                        )
                                    },
                                ]),
                            ]),
                        )
                    } else {
                        SemanticsRegion(
                            identifier: Some("product-more".to_string()),
                            child: Row(gap: tokens.spacing.m, children: [
                                copy("product.more").color(tokens.colors.text_secondary),
                                Link::to(
                                    cx.view.env().tr("product.all"),
                                    navigation::PRODUCTS,
                                ),
                            ]),
                        )
                    },
                ]),
            ),
            SiteFooter(),
        ]),
    )
    .width_length(Length::percent(100.0))
}
