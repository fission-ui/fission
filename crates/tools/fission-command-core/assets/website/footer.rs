use crate::design::message;
use crate::navigation::SiteLink;
use crate::state::WebsiteState;
use fission::prelude::*;

fission_component! {
    pub Footer<WebsiteState> {}

    view(_footer, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        Column(
            gap: tokens.spacing.s,
            children: [
                Text::new(message("footer.note"))
                    .color(tokens.colors.text_secondary)
                    .wrap(true),
                SiteLink::new(
                    "footer.attribution",
                    "https://fission.rs".to_string(),
                    "fission-attribution",
                ),
            ],
        ),
        identifier: Some("site-footer".to_string()),
    )
}
