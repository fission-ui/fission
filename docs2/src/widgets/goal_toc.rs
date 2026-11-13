use crate::content::{GoalDocument, GoalMeta};
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GoalToc<DocsState> {
        props {
            document: GoalDocument,
            tutorials: Vec<GoalMeta>,
        }
    }
    view(toc, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let current_order = toc.document.meta.order;
        let previous = toc.tutorials.iter().find(|tutorial| tutorial.order + 1 == current_order);
        let next = toc.tutorials.iter().find(|tutorial| tutorial.order == current_order + 1);
        let mut project_files = std::collections::BTreeMap::new();
        for step in &toc.document.steps {
            for path in step.files.keys().filter(|path| path.as_str() != "preview/lib.rs") {
                project_files.entry(path.clone()).or_insert_with(|| step.id.clone());
            }
        }
        let target = if toc.document.meta.run_command.contains("site serve") {
            cx.view.env().tr("goal.target.static")
        } else if toc.document.meta.run_command.contains("--target ssr") {
            cx.view.env().tr("goal.target.ssr")
        } else if toc.document.meta.run_command.contains("--target android") {
            cx.view.env().tr("goal.target.android")
        } else if toc.document.meta.run_command.contains("--target ios") {
            cx.view.env().tr("goal.target.ios")
        } else if toc.document.meta.run_command.contains("--target web") {
            cx.view.env().tr("goal.target.web")
        } else {
            cx.view.env().tr("goal.target.desktop")
        };
    } =>
    SemanticsRegion(
        identifier: Some("goal-right-nav".to_string()),
        label: Some(cx.view.env().tr("goal.path")),
        child: Column(gap: tokens.spacing.m, children: [
            copy("goal.path").weight(tokens.typography.font_weight_bold),
            SemanticsRegion(
                identifier: Some("goal-toc-primary".to_string()),
                child: Column(gap: tokens.spacing.s, children: [
                    Link::to(cx.view.env().tr("goal.build_title"), "#what-you-will-build"),
                    Link::to(cx.view.env().tr("goal.finished_title"), "#finished-example"),
                    for (index, step) in toc.document.steps.iter().enumerate() {
                        SemanticsRegion(
                            Link::to(format!("{}. {}", index + 1, step.title), format!("#{}", step.id)),
                            identifier: Some(if index == 0 { "goal-toc-current".to_string() } else { "goal-toc-step".to_string() }),
                            key: step.id.clone(),
                        )
                    },
                ]),
            ),
            SemanticsRegion(
                identifier: Some("goal-toc-secondary".to_string()),
                child: Column(gap: tokens.spacing.m, children: [
                    Divider(),
                    copy("goal.files").weight(tokens.typography.font_weight_bold),
                    for (path, step_id) in &project_files {
                        SemanticsRegion(
                            Link::to(path.clone(), format!("#{step_id}")),
                            identifier: Some("goal-project-file".to_string()),
                            key: path.clone(),
                        )
                    },
                    Divider(),
                    SemanticsRegion(
                        identifier: Some("goal-target-card".to_string()),
                        child: Column(gap: tokens.spacing.s, children: [
                            copy("goal.target_card").weight(tokens.typography.font_weight_bold),
                            Row(gap: tokens.spacing.s, children: [
                                Icon::svg(material::device::devices::regular()).size(tokens.spacing.m),
                                Column(children: [copy("goal.project_target").weight(tokens.typography.font_weight_semibold), Text(target)]),
                            ]),
                        ]),
                    ),
                    if let Some(previous) = previous {
                        SemanticsRegion(
                            identifier: Some("goal-previous-card".to_string()),
                            child: Column(gap: tokens.spacing.xs, children: [
                                copy("goal.previous"),
                                Link::to(previous.title.clone(), previous.path.clone()),
                            ]),
                        )
                    },
                    if let Some(next) = next {
                        SemanticsRegion(
                            identifier: Some("goal-next-card".to_string()),
                            child: Column(gap: tokens.spacing.xs, children: [
                                copy("goal.next"),
                                Link::to(next.title.clone(), next.path.clone()),
                            ]),
                        )
                    } else {
                        SemanticsRegion(
                            identifier: Some("goal-next-card".to_string()),
                            child: Column(gap: tokens.spacing.xs, children: [
                                copy("goal.next"),
                                Link::to(cx.view.env().tr("site.reference"), navigation::REFERENCE),
                            ]),
                        )
                    },
                ]),
            ),
        ]),
    )
}
