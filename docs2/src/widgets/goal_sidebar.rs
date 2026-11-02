use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub GoalSidebar<DocsState> {}
    view(_sidebar, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("goal-left-nav".to_string()),
        label: Some(cx.view.env().tr("goal.nav.label")),
        child: Column(gap: tokens.spacing.m, children: [
            copy("goal.nav.get_started").weight(tokens.typography.font_weight_bold),
            Link::to(cx.view.env().tr("goal.nav.introduction"), navigation::REFERENCE_OVERVIEW),
            Link::to(cx.view.env().tr("goal.nav.installation"), navigation::INSTALLATION),
            Link::to(cx.view.env().tr("goal.nav.first_app"), navigation::COUNTER_TUTORIAL),
            Divider(),
            copy("goal.nav.tutorials").weight(tokens.typography.font_weight_bold),
            SemanticsRegion(
                Link::to(cx.view.env().tr("goal.nav.counter"), navigation::COUNTER_TUTORIAL),
                identifier: Some("goal-nav-current".to_string()),
            ),
            Link::to(cx.view.env().tr("goal.nav.forms"), navigation::FORM_CONTROL),
            Link::to(cx.view.env().tr("goal.nav.routing"), navigation::ROUTER),
            Link::to(cx.view.env().tr("goal.nav.data"), navigation::REFERENCE),
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
