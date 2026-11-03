use crate::content::GoalMeta;
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{
    copy, DocsHeader, GuideCard, HubPathStep, HubSidebar, HubStarterCode, HubTaskCard,
};
use fission::prelude::*;

fission_component! {
    pub DocumentationHub<DocsState> {
        props { tutorials: Vec<GoalMeta> }
    }
    view(hub, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let popular_orders = [6, 7, 10, 9];
        let popular_tutorials: Vec<_> = popular_orders.iter().filter_map(|order| {
            hub.tutorials.iter().find(|tutorial| tutorial.order == *order)
        }).collect();
        let guide_titles = [
            "hub.guide.login.title",
            "hub.guide.protected.title",
            "hub.guide.admin.title",
            "hub.guide.inventory.title",
        ];
        let guide_descriptions = [
            "hub.guide.login.description",
            "hub.guide.protected.description",
            "hub.guide.admin.description",
            "hub.guide.inventory.description",
        ];
        let guide_tags = [
            vec!["Auth".to_string(), "Forms".to_string(), "Full stack".to_string()],
            vec!["Routing".to_string(), "Auth".to_string(), "Middleware".to_string()],
            vec!["Data".to_string(), "Tables".to_string(), "Layout".to_string()],
            vec!["SQLite".to_string(), "Offline".to_string(), "Deployment".to_string()],
        ];
    } =>
    Column(children: [
        DocsHeader::new(true),
        SemanticsRegion(
            identifier: Some("hub-shell".to_string()),
            child: Row(align_items: ir_op::AlignItems::Start, children: [
                HubSidebar(),
                SemanticsRegion(
                    identifier: Some("hub-main".to_string()),
                    child: Column(gap: tokens.spacing.l, children: [
                        SemanticsRegion(
                            identifier: Some("hub-breadcrumb".to_string()),
                            child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                                Icon::svg(material::action::home::regular()).size(tokens.spacing.m),
                                copy("hub.breadcrumb"),
                                Icon::svg(material::navigation::chevron_right::regular()).size(tokens.spacing.s),
                            ]),
                        ),
                        copy("hub.title").size(tokens.typography.display_md_size).weight(tokens.typography.font_weight_bold),
                        copy("hub.description").size(tokens.typography.body_large_size).color(tokens.colors.text_secondary),
                        SemanticsRegion(
                            identifier: Some("hub-path".to_string()),
                            child: Row(gap: tokens.spacing.l, wrap: ir_op::FlexWrap::Wrap, children: [
                                HubPathStep(1, "hub.path.first.title", "hub.path.first.detail"),
                                HubPathStep(2, "hub.path.interface.title", "hub.path.interface.detail"),
                                HubPathStep(3, "hub.path.data.title", "hub.path.data.detail"),
                                HubPathStep(4, "hub.path.target.title", "hub.path.target.detail"),
                                HubPathStep(5, "hub.path.ship.title", "hub.path.ship.detail"),
                            ]),
                        ),
                        SemanticsRegion(
                            identifier: Some("hub-start".to_string()),
                            child: Row(gap: tokens.spacing.xl, children: [
                                Column(gap: tokens.spacing.m, children: [
                                    copy("hub.start.label").color(tokens.colors.primary),
                                    copy("hub.start.title").size(tokens.typography.heading1_size).weight(tokens.typography.font_weight_bold),
                                    copy("hub.start.description").color(tokens.colors.text_secondary),
                                    Link::to(cx.view.env().tr("hub.start.action"), navigation::QUICKSTART),
                                ]),
                                HubStarterCode(),
                            ]),
                        ),
                        copy("hub.tasks").size(tokens.typography.heading2_size).weight(tokens.typography.font_weight_bold),
                        SemanticsRegion(
                            identifier: Some("hub-cards".to_string()),
                            child: Row(gap: tokens.spacing.m, children: [
                                HubTaskCard(
                                    material::action::article::regular(),
                                    "hub.interface",
                                    vec![("hub.interface.forms", navigation::FORM_CONTROL), ("hub.interface.menus", navigation::REFERENCE), ("hub.interface.layouts", navigation::REFERENCE)],
                                ),
                                HubTaskCard(
                                    material::device::storage::regular(),
                                    "hub.data",
                                    vec![("hub.data.async", "/docs/guides/resources-and-async/"), ("hub.data.sqlite", "/blog/build-a-crud-app-with-rust-and-sqlite/"), ("hub.data.apis", "/docs/guides/resources-and-async/")],
                                ),
                                HubTaskCard(
                                    material::hardware::smartphone::regular(),
                                    "hub.targets",
                                    vec![("hub.targets.desktop", "/docs/learn/examples-and-targets/"), ("hub.targets.mobile", "/docs/learn/examples-and-targets/"), ("hub.targets.web", "/docs/learn/examples-and-targets/"), ("hub.targets.sites", "/docs/guides/static-sites/")],
                                ),
                                HubTaskCard(
                                    material::action::rocket_launch::regular(),
                                    "hub.testing",
                                    vec![("hub.testing.flow", navigation::TEST_GUIDE), ("hub.testing.package", navigation::PACKAGING_GUIDE), ("hub.testing.deploy", navigation::PACKAGING_GUIDE)],
                                ),
                            ]),
                        ),
                        SemanticsRegion(
                            identifier: Some("hub-guides-heading".to_string()),
                            child: Row(align_items: ir_op::AlignItems::Center, children: [
                                copy("hub.guides").size(tokens.typography.heading2_size).weight(tokens.typography.font_weight_bold),
                                Spacer(flex_grow: 1.0),
                                Link::to(cx.view.env().tr("hub.all_guides"), navigation::TUTORIALS),
                            ]),
                        ),
                        SemanticsRegion(
                            identifier: Some("hub-guides".to_string()),
                            child: Row(gap: tokens.spacing.m, children: [
                                for (index, tutorial) in popular_tutorials.iter().enumerate() {
                                    GuideCard(
                                        match tutorial.order {
                                            6 => material::action::lock::regular(),
                                            7 => material::action::verified_user::regular(),
                                            10 => material::action::dashboard::regular(),
                                            _ => material::content::inventory_2::regular(),
                                        },
                                        cx.view.env().tr(guide_titles[index]),
                                        cx.view.env().tr(guide_descriptions[index]),
                                        tutorial.path.clone(),
                                        guide_tags[index].clone(),
                                        key: tutorial.path.clone(),
                                    )
                                },
                            ]),
                        ),
                        SemanticsRegion(
                            identifier: Some("hub-reference".to_string()),
                            child: Row(children: [
                                Column(gap: tokens.spacing.s, children: [
                                    copy("hub.reference").weight(tokens.typography.font_weight_bold),
                                    copy("hub.reference.body").color(tokens.colors.text_secondary),
                                ]),
                                Spacer(flex_grow: 1.0),
                                Link::to(cx.view.env().tr("hub.reference.action"), navigation::REFERENCE),
                            ]),
                        ),
                    ]),
                ),
            ]),
        ),
    ])
}
