use crate::content::GoalMeta;
use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GoalSidebar<DocsState> {
        props {
            current_path: String,
            tutorials: Vec<GoalMeta>,
        }
    }
    view(sidebar, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let current = sidebar.tutorials.iter().find(|tutorial| tutorial.path == sidebar.current_path);
    } =>
    SemanticsRegion(
        identifier: Some("goal-left-nav".to_string()),
        label: Some(cx.view.env().tr("goal.nav.label")),
        child: Column(gap: tokens.spacing.m, children: [
            copy("goal.nav.get_started").weight(tokens.typography.font_weight_bold),
            Link::to(cx.view.env().tr("goal.nav.introduction"), navigation::TUTORIALS),
            Link::to(cx.view.env().tr("goal.nav.installation"), navigation::INSTALLATION),
            Link::to(cx.view.env().tr("goal.nav.first_app"), navigation::FIRST_APP),
            Divider(),
            copy("goal.nav.tutorials").weight(tokens.typography.font_weight_bold),
            Link::to(cx.view.env().tr("tutorials.title"), navigation::TUTORIALS),
            if let Some(tutorial) = current {
                SemanticsRegion(
                    identifier: Some("goal-nav-current".to_string()),
                    label: Some(tutorial.title.clone()),
                    child: Text(tutorial.title.clone()).color(tokens.colors.text_secondary),
                )
            },
            Divider(),
            copy("goal.nav.concepts").weight(tokens.typography.font_weight_bold),
            Link::to(cx.view.env().tr("goal.nav.components"), "/reference/widgets/catalog/"),
            Link::to(cx.view.env().tr("goal.nav.state"), navigation::STATE_GUIDE),
            Link::to(cx.view.env().tr("goal.nav.routing"), navigation::ROUTER),
            Link::to(cx.view.env().tr("goal.nav.ssr"), "/docs/guides/server-sites/"),
            Link::to(cx.view.env().tr("goal.nav.styling"), "/docs/guides/theming-and-i18n/"),
            Link::to(cx.view.env().tr("goal.nav.testing"), navigation::TEST_GUIDE),
            Link::to(cx.view.env().tr("goal.nav.deployment"), navigation::PACKAGING_GUIDE),
            Divider(),
            copy("goal.nav.reference").weight(tokens.typography.font_weight_bold),
            Link::to(cx.view.env().tr("goal.nav.api"), navigation::REFERENCE),
            Link::to(cx.view.env().tr("goal.nav.cookbook"), navigation::COOKBOOK),
            Spacer(flex_grow: 1.0),
            SemanticsRegion(
                identifier: Some("goal-version-card".to_string()),
                child: Column(gap: tokens.spacing.s, children: [
                    copy("goal.version").weight(tokens.typography.font_weight_bold),
                    copy("goal.latest").color(tokens.colors.text_secondary),
                    Link::to(cx.view.env().tr("goal.changelog"), navigation::LATEST_RELEASE),
                ]),
            ),
        ]),
    )
}
