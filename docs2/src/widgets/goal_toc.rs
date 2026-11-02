use crate::content::GoalDocument;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GoalToc<DocsState> {
        props { document: GoalDocument }
    }
    view(toc, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("goal-right-nav".to_string()),
        label: Some(cx.view.env().tr("goal.path")),
        child: Column(gap: tokens.spacing.m, children: [
            copy("goal.path").weight(tokens.typography.font_weight_bold),
            Link::to(cx.view.env().tr("goal.build_title"), "#what-you-will-build"),
            Link::to(cx.view.env().tr("goal.finished_title"), "#finished-example"),
            for (index, step) in toc.document.steps.iter().enumerate() {
                SemanticsRegion(
                    Link::to(format!("{}. {}", index + 1, step.title), format!("#{}", step.id)),
                    identifier: Some(if index == 0 { "goal-toc-current".to_string() } else { "goal-toc-step".to_string() }),
                    key: step.id.clone(),
                )
            },
            Divider(),
            copy("goal.files"),
            copy("goal.next"),
        ]),
    )
}
