use crate::content::GoalStep;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GoalStepSection<DocsState> {
        props {
            app: String,
            index: usize,
            step: GoalStep,
        }
    }
    view(section, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let preview_key = format!("goal-preview-{}", section.step.id);
        let preview_id = WidgetId::explicit(&preview_key);
        let section_identifier = format!("site-section:{}", section.step.id);
        let heading = format!("{}. {}", section.index + 1, section.step.title);
        let preview_label = format!("{}: {}", cx.view.env().tr("goal.live"), section.step.title);
    } =>
    SemanticsRegion(
        identifier: Some(section_identifier),
        label: Some(heading.clone()),
        child: Column(gap: tokens.spacing.m, children: [
            Text(heading)
                .size(tokens.typography.heading1_size)
                .weight(tokens.typography.font_weight_bold),
            Row(gap: tokens.spacing.l, align_items: ir_op::AlignItems::Stretch, children: [
                SemanticsRegion(
                    MarkdownContent::new(section.step.markdown.clone()),
                    identifier: Some("goal-step-copy".to_string()),
                ),
                Column(gap: tokens.spacing.s, children: [
                    SemanticsRegion(
                        identifier: Some("goal-live-label".to_string()),
                        child: Row(gap: tokens.spacing.s, children: [
                            Icon::svg(material::av::play_arrow::regular())
                                .size(tokens.spacing.m)
                                .color(tokens.colors.primary),
                            copy("goal.live").weight(tokens.typography.font_weight_bold),
                            Spacer(flex_grow: 1.0),
                            copy("goal.compiled").color(tokens.colors.text_secondary),
                        ]),
                    ),
                    SemanticsRegion(
                        identifier: Some("goal-live-preview".to_string()),
                        child: WebView(
                            id: preview_id,
                            url: section.step.preview_url(&section.app),
                            label: Some(preview_label),
                            height: Some(360.0),
                        ),
                    ),
                    if let Some(checkpoint) = &section.step.checkpoint {
                        SemanticsRegion(
                            identifier: Some("goal-checkpoint".to_string()),
                            label: Some(cx.view.env().tr("goal.checkpoint")),
                            child: Row(gap: tokens.spacing.m, align_items: ir_op::AlignItems::Start, children: [
                                Icon::svg(material::action::info::regular())
                                    .size(tokens.spacing.l)
                                    .color(tokens.colors.primary),
                                MarkdownContent::new(checkpoint.clone()),
                            ]),
                        )
                    },
                ]),
            ]),
        ]),
        key: section.step.id.clone(),
    )
}
