use crate::content::GoalMeta;
use crate::state::DocsState;
use crate::widgets::{copy, GoalCard};
use fission::prelude::*;

fission_component! {
    pub GoalChooser<DocsState> {
        props { tutorials: Vec<GoalMeta> }
    }
    view(chooser, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let desktop = chooser.tutorials.iter().find(|tutorial| tutorial.order == 12)
            .expect("desktop tutorial metadata is present");
        let mobile = chooser.tutorials.iter().find(|tutorial| tutorial.order == 13)
            .expect("mobile tutorial metadata is present");
        let web = chooser.tutorials.iter().find(|tutorial| tutorial.order == 16)
            .expect("Web tutorial metadata is present");
    } =>
    SemanticsRegion(
        identifier: Some("home-goals".to_string()),
        label: Some(cx.view.env().tr("home.goal.title")),
        child: Column(
            gap: tokens.spacing.m,
            children: [
                SemanticsRegion(
                    copy("home.goal.title")
                        .weight(tokens.typography.font_weight_bold),
                    identifier: Some("site-heading-2:choose-a-project".to_string()),
                ),
                Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Stretch, children: [
                    GoalCard(material::hardware::desktop_windows::regular(), "home.goal.desktop", "home.goal.desktop_body", "home.goal.desktop_action", "/img/examples/widget-gallery.png", desktop.path.clone()),
                    GoalCard(material::hardware::smartphone::regular(), "home.goal.mobile", "home.goal.mobile_body", "home.goal.mobile_action", "/img/examples/inbox-compact.png", mobile.path.clone()),
                    GoalCard(material::action::language::regular(), "home.goal.web", "home.goal.web_body", "home.goal.web_action", "/img/examples/inbox.png", web.path.clone()),
                ]),
            ],
        ),
    )
}
