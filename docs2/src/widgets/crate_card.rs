use crate::registry::RegistryCrate;
use crate::state::DocsState;
use fission::prelude::*;

fission_component! {
    pub CrateCard<DocsState> {
        props { package: RegistryCrate }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        Column(gap: tokens.spacing.s, children: [
            Link::to(card.package.name.clone(), format!("/crates/{}/", card.package.name)),
            Text(card.package.version.clone()).family(tokens.typography.font_family_mono.clone()),
            Text(card.package.description.clone()).wrap(true),
            Text(card.package.platforms.join(" · ")).wrap(true).color(tokens.colors.text_secondary),
            if card.package.is_prerelease() || card.package.api_status == "alpha" {
                Text(TextContent::Key("crates.alpha".into()))
            },
        ]),
    )
    .padding_all(tokens.spacing.l)
    .bg(tokens.colors.surface)
}
