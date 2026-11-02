use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, FeatureCard};
use fission::prelude::*;

fission_component! {
    pub FeatureSection<DocsState> {}
    view(_section, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-features".to_string()),
        label: Some(cx.view.env().tr("home.ready.title")),
        child: Row(
            gap: tokens.spacing.xl,
            align_items: ir_op::AlignItems::Stretch,
            children: [
                Column(gap: tokens.spacing.m, children: [
                    copy("home.ready.eyebrow")
                        .color(tokens.colors.text_secondary)
                        .weight(tokens.typography.font_weight_semibold),
                    copy("home.ready.title")
                        .size(tokens.typography.heading1_size)
                        .weight(tokens.typography.font_weight_bold),
                    copy("home.ready.summary").color(tokens.colors.text_secondary),
                ]),
                FeatureCard(material::action::article::regular(), "home.ready.widgets", "home.ready.widgets_body", "home.ready.widgets_link", navigation::WIDGETS_GUIDE),
                FeatureCard(material::action::verified::regular(), "home.ready.state", "home.ready.state_body", "home.ready.state_link", navigation::STATE_GUIDE),
                FeatureCard(material::action::language::regular(), "home.ready.routing", "home.ready.routing_body", "home.ready.routing_link", navigation::ROUTER),
                FeatureCard(material::action::verified::regular(), "home.ready.testing", "home.ready.testing_body", "home.ready.testing_link", navigation::TEST_GUIDE),
                FeatureCard(material::action::accessibility_new::regular(), "home.ready.accessibility", "home.ready.accessibility_body", "home.ready.accessibility_link", navigation::ACCESSIBILITY_GUIDE),
                FeatureCard(material::action::rocket_launch::regular(), "home.ready.packaging", "home.ready.packaging_body", "home.ready.packaging_link", navigation::PACKAGING_GUIDE),
            ],
        ),
    )
}
