use crate::navigation;
use crate::state::DocsState;
use crate::widgets::{copy, GoalCard};
use fission::prelude::*;

fission_component! {
    pub GoalChooser<DocsState> {}
    view(_chooser, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-goals".to_string()),
        label: Some(cx.view.env().tr("home.goal.title")),
        child: Row(
            gap: tokens.spacing.xl,
            align_items: ir_op::AlignItems::Stretch,
            children: [
                Column(gap: tokens.spacing.m, children: [
                    copy("home.goal.eyebrow")
                        .color(tokens.colors.text_secondary)
                        .weight(tokens.typography.font_weight_semibold),
                    copy("home.goal.title")
                        .size(tokens.typography.heading1_size)
                        .weight(tokens.typography.font_weight_bold),
                    copy("home.goal.summary").color(tokens.colors.text_secondary),
                ]),
                Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Stretch, children: [
                    GoalCard(material::hardware::desktop_windows::regular(), "home.goal.desktop", "home.goal.desktop_body", "/img/examples/editor.png", navigation::REFERENCE_OVERVIEW),
                    GoalCard(material::hardware::smartphone::regular(), "home.goal.mobile", "home.goal.mobile_body", "/img/examples/inbox-compact.png", navigation::REFERENCE_OVERVIEW),
                    GoalCard(material::action::language::regular(), "home.goal.web", "home.goal.web_body", "/img/examples/inbox.png", navigation::REFERENCE_OVERVIEW),
                ]),
            ],
        ),
    )
}
