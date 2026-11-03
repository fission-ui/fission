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
    } =>
    SemanticsRegion(
        identifier: Some("goal-left-nav".to_string()),
        label: Some(cx.view.env().tr("goal.nav.label")),
        child: Column(gap: tokens.spacing.m, children: [
            copy("goal.nav.get_started").weight(tokens.typography.font_weight_bold),
            Link::to(cx.view.env().tr("goal.nav.introduction"), navigation::REFERENCE_OVERVIEW),
            Link::to(cx.view.env().tr("goal.nav.installation"), navigation::INSTALLATION),
            Link::to(cx.view.env().tr("goal.nav.first_app"), navigation::QUICKSTART),
            Divider(),
            copy("goal.nav.tutorials").weight(tokens.typography.font_weight_bold),
            for tutorial in &sidebar.tutorials {
                SemanticsRegion(
                    Link::to(tutorial.title.clone(), tutorial.path.clone()),
                    identifier: Some(if tutorial.path == sidebar.current_path {
                        "goal-nav-current".to_string()
                    } else {
                        format!("goal-nav-{}", tutorial.app)
                    }),
                    key: tutorial.path.clone(),
                )
            },
            Link::to(cx.view.env().tr("tutorials.title"), navigation::TUTORIALS),
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
                    Link::to(cx.view.env().tr("goal.changelog"), navigation::CHANGELOG),
                ]),
            ),
        ]),
    )
}
