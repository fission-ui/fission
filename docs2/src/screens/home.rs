use crate::content::GoalMeta;
use crate::state::DocsState;
use crate::widgets::{
    BrowserCta, ComposeSection, FeatureSection, GoalChooser, HomeHero, PlatformStrip, SiteFooter,
    SiteHeader, TutorialGallery,
};
use fission::prelude::*;

fission_component! {
    pub HomePage<DocsState> {
        props {
            tutorials: Vec<GoalMeta>,
            hero_source: String,
            hero_preview_url: String,
        }
    }
    view(home, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("home-page".to_string()),
        child: Container(
            Column(children: [
                SiteHeader(),
                HomeHero(home.hero_source.clone(), home.hero_preview_url.clone()),
                PlatformStrip(),
                GoalChooser(home.tutorials.clone()),
                FeatureSection(),
                ComposeSection(),
                TutorialGallery(home.tutorials.clone()),
                BrowserCta(),
                SiteFooter(),
            ]),
        )
        .width_length(Length::percent(100.0))
        .bg(tokens.colors.background),
    )
}
