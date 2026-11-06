use crate::content::GoalDocument;
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GoalIntro<DocsState> {
        props { document: GoalDocument }
    }
    view(intro, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let document = &intro.document;
        let finished = document.steps.last().expect("goal has at least one step");
        let preview_key = format!("goal-finished-{}", document.meta.app);
        let preview_id = WidgetId::explicit(&preview_key);
        let run_command = &document.meta.run_command;
        let target_label = if run_command.contains("site serve") {
            cx.view.env().tr("goal.target.static")
        } else if run_command.contains("--target ssr") {
            cx.view.env().tr("goal.target.ssr")
        } else if run_command.contains("--target android") {
            cx.view.env().tr("goal.target.android")
        } else if run_command.contains("--target ios") {
            cx.view.env().tr("goal.target.ios")
        } else if run_command.contains("--target web") {
            cx.view.env().tr("goal.target.web")
        } else {
            cx.view.env().tr("goal.target.desktop")
        };
        let run_command_block = format!("```sh\n{run_command}\n```");
    } =>
    Column(gap: tokens.spacing.l, children: [
        SemanticsRegion(
            identifier: Some("site-navigation".to_string()),
            label: Some(cx.view.env().tr("goal.breadcrumb.label")),
            child: Row(gap: tokens.spacing.s, align_items: ir_op::AlignItems::Center, children: [
                Link::to(cx.view.env().tr("site.home"), navigation::HOME),
                Icon::svg(material::navigation::chevron_right::regular()).size(tokens.spacing.s),
                Link::to(cx.view.env().tr("site.tutorials"), navigation::TUTORIALS),
                Icon::svg(material::navigation::chevron_right::regular()).size(tokens.spacing.s),
                Text(document.meta.title.clone()),
            ]),
        ),
        Row(align_items: ir_op::AlignItems::Start, wrap: ir_op::FlexWrap::Wrap, children: [
            SemanticsRegion(
                identifier: Some("goal-heading".to_string()),
                child: Column(gap: tokens.spacing.s, children: [
                    SemanticsRegion(
                        Text(document.meta.title.clone())
                            .weight(tokens.typography.font_weight_bold),
                        identifier: Some("site-heading-1:goal".to_string()),
                    ),
                    Text(document.meta.description.clone())
                        .color(tokens.colors.text_secondary),
                ]),
            ),
            Spacer(flex_grow: 1.0),
            SemanticsRegion(
                identifier: Some("goal-meta".to_string()),
                child: Badge(text: target_label),
            ),
        ]),
        SemanticsRegion(
            identifier: Some("site-section:what-you-will-build".to_string()),
            child: Column(gap: tokens.spacing.m, children: [
                SemanticsRegion(
                    copy("goal.build_title")
                        .weight(tokens.typography.font_weight_bold),
                    identifier: Some("site-heading-2:what-you-will-build-heading".to_string()),
                ),
                Text(document.meta.goal.clone()),
                MarkdownContent::new(document.introduction.clone()),
            ]),
        ),
        if let Some(project_setup) = &document.project_setup {
            SemanticsRegion(
                identifier: Some("site-disclosure:goal-project-setup".to_string()),
                child: Column(children: [
                    SemanticsRegion(
                        identifier: Some("site-disclosure-summary:goal-project-setup".to_string()),
                        child: Some(copy("goal.project_setup").into()),
                    ),
                    SemanticsRegion(
                        identifier: Some("goal-project-setup-content".to_string()),
                        child: Some(MarkdownContent::new(project_setup.clone()).into()),
                    ),
                ]),
            )
        },
        SemanticsRegion(
            identifier: Some("site-section:finished-example".to_string()),
            child: Column(gap: tokens.spacing.m, children: [
                SemanticsRegion(
                    copy("goal.finished_title")
                        .weight(tokens.typography.font_weight_bold),
                    identifier: Some("site-heading-2:finished-example-heading".to_string()),
                ),
                SemanticsRegion(
                    identifier: Some("goal-command".to_string()),
                    child: Some(MarkdownContent::new(run_command_block).into()),
                ),
                if finished.preview {
                    SemanticsRegion(
                        identifier: Some("goal-preview-disclosure".to_string()),
                        child: Column(gap: tokens.spacing.m, children: [
                            SemanticsRegion(
                                identifier: Some("site-client-action:toggle-goal-preview".to_string()),
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
                                identifier: Some("goal-finished-preview".to_string()),
                                child: WebView(
                                    id: preview_id,
                                    url: finished.preview_url(&document.meta.app),
                                    label: Some(format!("App preview for {}", document.meta.title)),
                                    height: Some(560.0),
                                ),
                            ),
                        ]),
                    )
                },
            ]),
        ),
    ])
}
