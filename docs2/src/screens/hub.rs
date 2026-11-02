use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, SiteHeader};
use fission::prelude::*;

fission_component! {
    pub DocumentationHub<DocsState> {}
    view(_hub, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Column(children: [
        SiteHeader(),
        SemanticsRegion(
            identifier: Some("hub-shell".to_string()),
            child: Row(align_items: ir_op::AlignItems::Start, children: [
                SemanticsRegion(
                    identifier: Some("hub-sidebar".to_string()),
                    label: Some(cx.view.env().tr("goal.nav.label")),
                    child: Column(gap: tokens.spacing.l, children: [
                        Link::to(cx.view.env().tr("hub.home"), navigation::DOCS),
                        Link::to(cx.view.env().tr("goal.nav.first_app"), navigation::COUNTER_TUTORIAL),
                        Link::to(cx.view.env().tr("site.tutorials"), navigation::COUNTER_TUTORIAL),
                        Link::to(cx.view.env().tr("site.reference"), navigation::REFERENCE_OVERVIEW),
                        Divider(),
                        Link::to(cx.view.env().tr("site.examples"), navigation::SHOWCASE),
                        Link::to(cx.view.env().tr("site.github"), navigation::GITHUB),
                    ]),
                ),
                SemanticsRegion(
                    identifier: Some("hub-main".to_string()),
                    child: Column(gap: tokens.spacing.l, children: [
                        copy("hub.breadcrumb").color(tokens.colors.text_secondary),
                        copy("hub.title").size(tokens.typography.display_md_size).weight(tokens.typography.font_weight_bold),
                        copy("hub.description").size(tokens.typography.body_large_size).color(tokens.colors.text_secondary),
                        SemanticsRegion(
                            identifier: Some("hub-path".to_string()),
                            child: Row(gap: tokens.spacing.l, wrap: ir_op::FlexWrap::Wrap, children: [
                                copy("hub.path.first"), copy("hub.path.interface"),
                                copy("hub.path.data"), copy("hub.path.target"), copy("hub.path.ship"),
                            ]),
                        ),
                        SemanticsRegion(
                            identifier: Some("hub-start".to_string()),
                            child: Row(gap: tokens.spacing.xl, children: [
                                Column(gap: tokens.spacing.m, children: [
                                    copy("hub.start.label").color(tokens.colors.primary),
                                    copy("hub.start.title").size(tokens.typography.heading1_size).weight(tokens.typography.font_weight_bold),
                                    copy("hub.start.description").color(tokens.colors.text_secondary),
                                    Link::to(cx.view.env().tr("hub.start.action"), navigation::COUNTER_TUTORIAL),
                                ]),
                                Code(text: "fission init my-app\ncd my-app\nfission run".to_string()),
                            ]),
                        ),
                        copy("hub.tasks").size(tokens.typography.heading2_size).weight(tokens.typography.font_weight_bold),
                        SemanticsRegion(
                            identifier: Some("hub-cards".to_string()),
                            child: Row(gap: tokens.spacing.m, children: [
                                HubCard(material::action::article::regular(), "hub.interface", "hub.interface.body", navigation::FORM_CONTROL),
                                HubCard(material::action::verified::regular(), "hub.data", "hub.data.body", navigation::REFERENCE),
                                HubCard(material::hardware::smartphone::regular(), "hub.targets", "hub.targets.body", navigation::REFERENCE_OVERVIEW),
                                HubCard(material::action::rocket_launch::regular(), "hub.testing", "hub.testing.body", navigation::COOKBOOK),
                            ]),
                        ),
                        copy("hub.guides").size(tokens.typography.heading2_size).weight(tokens.typography.font_weight_bold),
                        SemanticsRegion(
                            identifier: Some("hub-guides".to_string()),
                            child: Row(gap: tokens.spacing.m, children: [
                                HubCard(material::action::verified::regular(), "home.learn.counter", "home.learn.counter_body", navigation::COUNTER_TUTORIAL),
                                HubCard(material::action::article::regular(), "goal.nav.forms", "hub.interface.body", navigation::FORM_CONTROL),
                                HubCard(material::action::language::regular(), "goal.nav.routing", "hub.targets.body", navigation::ROUTER),
                                HubCard(material::action::article::regular(), "site.reference", "hub.reference.body", navigation::REFERENCE),
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

fission_component! {
    HubCard<DocsState> {
        props { icon: &'static str, title: &'static str, body: &'static str, href: &'static str }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("hub-card".to_string()),
        child: Column(gap: tokens.spacing.m, children: [
            Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Center, children: [
                Icon::svg(card.icon).size(tokens.spacing.xl).color(tokens.colors.primary),
                copy(card.title).weight(tokens.typography.font_weight_bold),
            ]),
            copy(card.body).color(tokens.colors.text_secondary),
            Link::to(cx.view.env().tr("hub.open"), card.href),
        ]),
    )
}
