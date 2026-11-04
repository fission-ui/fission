use crate::content::GoalMeta;
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{
    copy, GuideCard, HubPathStep, HubSidebar, HubStarterCode, HubTaskCard, SiteHeader,
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
        SiteHeader(),
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
                        SemanticsRegion(
                            copy("hub.title").weight(tokens.typography.font_weight_bold),
                            identifier: Some("site-heading-1:docs-hub".to_string()),
                        ),
                        copy("hub.description").color(tokens.colors.text_secondary),
                        SemanticsRegion(
                            identifier: Some("hub-path".to_string()),
                            child: Row(gap: tokens.spacing.l, wrap: ir_op::FlexWrap::Wrap, children: [
                                HubPathStep(1, "hub.path.first.title", "hub.path.first.detail", navigation::FIRST_APP),
                                HubPathStep(2, "hub.path.interface.title", "hub.path.interface.detail", navigation::LOGIN_PAGE),
                                HubPathStep(3, "hub.path.data.title", "hub.path.data.detail", navigation::CRUD_APP),
                                HubPathStep(4, "hub.path.target.title", "hub.path.target.detail", navigation::MOBILE_APPS),
                                HubPathStep(5, "hub.path.ship.title", "hub.path.ship.detail", navigation::ANDROID_RELEASE),
                            ]),
                        ),
                        SemanticsRegion(
                            identifier: Some("hub-start".to_string()),
                            child: Row(gap: tokens.spacing.xl, children: [
                                Column(gap: tokens.spacing.m, children: [
                                    copy("hub.start.label").color(tokens.colors.primary),
                                    SemanticsRegion(
                                        copy("hub.start.title").weight(tokens.typography.font_weight_bold),
                                        identifier: Some("site-heading-2:hub-start".to_string()),
                                    ),
                                    copy("hub.start.description").color(tokens.colors.text_secondary),
                                    Link::to(cx.view.env().tr("hub.start.action"), navigation::FIRST_APP),
                                ]),
                                HubStarterCode(),
                            ]),
                        ),
                        SemanticsRegion(
                            copy("hub.tasks").weight(tokens.typography.font_weight_bold),
                            identifier: Some("site-heading-2:hub-tasks".to_string()),
                        ),
                        SemanticsRegion(
                            identifier: Some("hub-cards".to_string()),
                            child: Row(gap: tokens.spacing.m, children: [
                                HubTaskCard(
                                    material::action::article::regular(),
                                    "hub.interface",
                                    vec![("hub.interface.forms", navigation::LOGIN_PAGE), ("hub.interface.menus", navigation::WEBSITE_NAVIGATION), ("hub.interface.layouts", navigation::ANDROID_RESPONSIVE_LAYOUTS)],
                                ),
                                HubTaskCard(
                                    material::device::storage::regular(),
                                    "hub.data",
                                    vec![("hub.data.async", navigation::CRUD_APP), ("hub.data.sqlite", navigation::CRUD_APP), ("hub.data.apis", navigation::PRODUCT_INVENTORY)],
                                ),
                                HubTaskCard(
                                    material::hardware::smartphone::regular(),
                                    "hub.targets",
                                    vec![("hub.targets.desktop", navigation::DESKTOP_APP), ("hub.targets.mobile", navigation::MOBILE_APPS), ("hub.targets.web", navigation::FIRST_APP), ("hub.targets.sites", navigation::DOCUMENTATION_SITE)],
                                ),
                                HubTaskCard(
                                    material::action::rocket_launch::regular(),
                                    "hub.testing",
                                    vec![("hub.testing.flow", navigation::LIVE_TEST), ("hub.testing.package", navigation::ANDROID_RELEASE), ("hub.testing.deploy", navigation::ANDROID_RELEASE)],
                                ),
                            ]),
                        ),
                        SemanticsRegion(
                            identifier: Some("hub-guides-heading".to_string()),
                            child: Row(align_items: ir_op::AlignItems::Center, children: [
                                SemanticsRegion(
                                    copy("hub.guides").weight(tokens.typography.font_weight_bold),
                                    identifier: Some("site-heading-2:hub-guides".to_string()),
                                ),
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
