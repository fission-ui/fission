use crate::content::GoalDocument;
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
    } =>
    Column(gap: tokens.spacing.l, children: [
        Text(TextContent::Key("goal.breadcrumb".into()))
            .color(tokens.colors.text_secondary),
        Row(align_items: ir_op::AlignItems::End, children: [
            SemanticsRegion(
                identifier: Some("goal-heading".to_string()),
                child: Column(gap: tokens.spacing.s, children: [
                    Text(document.meta.title.clone())
                        .size(tokens.typography.display_md_size)
                        .weight(tokens.typography.font_weight_bold),
                    Text(document.meta.description.clone())
                        .size(tokens.typography.body_large_size)
                        .color(tokens.colors.text_secondary),
                ]),
            ),
            Spacer(flex_grow: 1.0),
            SemanticsRegion(
                identifier: Some("goal-meta".to_string()),
                child: Column(gap: tokens.spacing.s, children: [
                    copy("goal.checked").color(tokens.colors.text_secondary),
                    Row(gap: tokens.spacing.s, children: [
                        Badge(text: cx.view.env().tr("goal.target.preview")),
                        Badge(text: target_label),
                    ]),
                    Alert(
                        title: cx.view.env().tr("goal.tests"),
                        kind: AlertKind::Success,
                    ),
                ]),
            ),
        ]),
        SemanticsRegion(
            identifier: Some("site-section:what-you-will-build".to_string()),
            child: Row(gap: tokens.spacing.l, align_items: ir_op::AlignItems::Stretch, children: [
                    if finished.preview {
                        SemanticsRegion(
                            identifier: Some("goal-finished-preview".to_string()),
                            child: Column(children: [
                                Row(children: [Text("●  ●  ●"), Spacer(flex_grow: 1.0), copy("goal.localhost")]),
                                WebView(
                                    id: preview_id,
                                    url: finished.preview_url(&document.meta.app),
                                    label: Some(format!("Finished {} app", document.meta.title)),
                                    height: Some(300.0),
                                ),
                            ]),
                        )
                    },
                SemanticsRegion(
                    identifier: Some("goal-outcomes".to_string()),
                    child: Column(gap: tokens.spacing.m, children: [
                        copy("goal.build_title")
                            .size(tokens.typography.heading1_size)
                            .weight(tokens.typography.font_weight_bold),
                        for outcome in &document.meta.outcomes {
                            Row(gap: tokens.spacing.m, children: [
                                Icon::svg(material::navigation::check::regular()).size(tokens.spacing.m).color(tokens.colors.primary),
                                Text(outcome.clone()),
                            ], key: outcome.clone())
                        },
                        Row(gap: tokens.spacing.m, children: [
                            Icon::svg(material::navigation::check::regular()).size(tokens.spacing.m).color(tokens.colors.primary),
                            copy("goal.browser_outcome"),
                        ]),
                    ]),
                ),
            ]),
        ),
        SemanticsRegion(
            identifier: Some("site-section:finished-example".to_string()),
            child: Column(gap: tokens.spacing.m, children: [
                copy("goal.finished_title")
                    .size(tokens.typography.heading1_size)
                    .weight(tokens.typography.font_weight_bold),
                copy("goal.finished_summary").color(tokens.colors.text_secondary),
                Row(gap: tokens.spacing.l, align_items: ir_op::AlignItems::Stretch, children: [
                    SemanticsRegion(
                        identifier: Some("goal-command".to_string()),
                        child: Code(text: document.meta.run_command.clone()),
                    ),
                    SemanticsRegion(
                        identifier: Some("goal-expected".to_string()),
                        child: Column(gap: tokens.spacing.s, children: [
                            copy("goal.expected").weight(tokens.typography.font_weight_bold),
                            Text(document.meta.goal.clone()).color(tokens.colors.text_secondary),
                        ]),
                    ),
                ]),
            ]),
        ),
    ])
}
