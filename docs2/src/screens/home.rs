use crate::state::DocsState;
use crate::widgets::{
    BrowserCta, ComposeSection, FeatureSection, GoalChooser, HomeHero, PlatformStrip, SiteFooter,
    SiteHeader, TutorialGallery,
};
use fission::prelude::*;

fission_component! {
    pub HomePage<DocsState> {}
    view(_home, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        Column(children: [
            SiteHeader(),
            HomeHero(),
            PlatformStrip(),
            GoalChooser(),
            FeatureSection(),
            ComposeSection(),
            TutorialGallery(),
            BrowserCta(),
            SiteFooter(),
        ]),
    )
    .width_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}
