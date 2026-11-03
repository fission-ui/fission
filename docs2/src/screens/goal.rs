use crate::content::{GoalDocument, GoalMeta};
use crate::state::DocsState;
use crate::widgets::{DocsHeader, GoalIntro, GoalSidebar, GoalStepSection, GoalToc, SiteFooter};
use fission::prelude::*;

fission_component! {
    pub GoalPage<DocsState> {
        props {
            document: GoalDocument,
            tutorials: Vec<GoalMeta>,
        }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        Column(children: [
            DocsHeader::new(false),
            SemanticsRegion(
                identifier: Some("goal-shell".to_string()),
                child: Row(align_items: ir_op::AlignItems::Start, children: [
                    GoalSidebar(page.document.meta.path.clone(), page.tutorials.clone()),
                    SemanticsRegion(
                        identifier: Some("goal-main".to_string()),
                        child: Column(gap: tokens.spacing.xl, children: [
                            GoalIntro(page.document.clone()),
                            for (index, step) in page.document.steps.iter().enumerate() {
                                GoalStepSection(
                                    page.document.meta.app.clone(),
                                    index,
                                    step.clone(),
                                    key: step.id.clone(),
                                )
                            },
                        ]),
                    ),
                    GoalToc(page.document.clone(), page.tutorials.clone()),
                ]),
            ),
            SiteFooter(),
        ]),
    )
    .width_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}
