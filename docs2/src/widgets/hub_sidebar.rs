use crate::navigation;
use crate::state::DocsState;
use crate::widgets::copy;
use fission::prelude::*;

fission_component! {
    pub HubSidebar<DocsState> {}
    view(_sidebar, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("hub-sidebar".to_string()),
        label: Some(cx.view.env().tr("goal.nav.label")),
        child: Column(gap: tokens.spacing.s, children: [
            SemanticsRegion(
                identifier: Some("hub-nav-selected".to_string()),
                child: Some(Link::to(cx.view.env().tr("hub.home"), navigation::DOCS).into()),
            ),
            copy("hub.nav.get_started").color(tokens.colors.text_secondary),
            Link::to(cx.view.env().tr("goal.nav.introduction"), navigation::REFERENCE_OVERVIEW),
            Link::to(cx.view.env().tr("goal.nav.installation"), navigation::INSTALLATION),
            Link::to(cx.view.env().tr("goal.nav.first_app"), navigation::QUICKSTART),
            Link::to(cx.view.env().tr("hub.nav.guides"), navigation::TUTORIALS),
            Link::to(cx.view.env().tr("hub.nav.targets"), navigation::REFERENCE_OVERVIEW),
            Link::to(cx.view.env().tr("hub.nav.recipes"), navigation::COOKBOOK),
            Link::to(cx.view.env().tr("goal.nav.api"), navigation::REFERENCE),
            Divider(),
            copy("hub.nav.background").color(tokens.colors.text_secondary),
            Link::to(cx.view.env().tr("hub.nav.new"), navigation::REFERENCE_OVERVIEW),
            Link::to(cx.view.env().tr("hub.nav.react"), "/docs/fission/for/react-developers/"),
            Link::to(cx.view.env().tr("hub.nav.flutter"), "/docs/fission/for/flutter-developers/"),
            Divider(),
            copy("hub.nav.more").color(tokens.colors.text_secondary),
            Link::to(cx.view.env().tr("site.examples"), navigation::EXAMPLES),
            Link::to(cx.view.env().tr("docs.header.community"), navigation::COMMUNITY),
        ]),
    )
}
