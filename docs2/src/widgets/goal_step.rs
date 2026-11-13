use crate::content::GoalStep;
use crate::state::DocsState;
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
        let preview_key = format!("goal-preview-{}-{}", section.app, section.step.id);
        let preview_id = WidgetId::explicit(&preview_key);
        let section_identifier = format!("site-section:{}", section.step.id);
        let heading_identifier = format!("site-heading-2:{}-heading", section.step.id);
        let heading = format!("{}. {}", section.index + 1, section.step.title);
        let preview_label = format!("{}: {}", cx.view.env().tr("goal.live"), section.step.title);
    } =>
    SemanticsRegion(
        identifier: Some(section_identifier),
        label: Some(heading.clone()),
        child: Column(gap: tokens.spacing.m, children: [
            SemanticsRegion(
                Text(heading).weight(tokens.typography.font_weight_bold),
                identifier: Some(heading_identifier),
            ),
            SemanticsRegion(
                MarkdownContent::new(section.step.markdown.clone()),
                identifier: Some("goal-step-copy".to_string()),
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
            if section.step.preview {
                SemanticsRegion(
                    identifier: Some("site-disclosure:goal-preview".to_string()),
                    child: Column(gap: tokens.spacing.m, children: [
                        SemanticsRegion(
                            identifier: Some("site-disclosure-summary:goal-preview".to_string()),
                            label: Some(cx.view.env().tr("goal.preview.show")),
                            child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                                Icon::svg(material::av::play_arrow::regular())
                                    .size(tokens.spacing.m)
                                    .color(tokens.colors.primary),
                                SemanticsRegion(
                                    identifier: Some("goal-preview-show-label".to_string()),
                                    child: Text(cx.view.env().tr("goal.preview.show")),
                                ),
                                SemanticsRegion(
                                    identifier: Some("goal-preview-hide-label".to_string()),
                                    child: Text(cx.view.env().tr("goal.preview.hide")),
                                ),
                            ]),
                        ),
                        SemanticsRegion(
                            identifier: Some("goal-live-preview".to_string()),
                            child: WebView(
                                id: preview_id,
                                url: section.step.preview_url(&section.app),
                                label: Some(preview_label),
                                height: Some(560.0),
                            ),
                        ),
                    ]),
                )
            },
        ]),
        key: section.step.id.clone(),
    )
}
