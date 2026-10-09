use crate::design::message;
use crate::navigation::SiteLink;
use fission::prelude::*;

#[derive(Clone)]
pub struct Footer;

impl From<Footer> for Widget {
    fn from(_: Footer) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        SemanticsRegion::new(Column {
            gap: Some(tokens.spacing.s),
            children: vec![
                Text::new(message("footer.note"))
                    .color(tokens.colors.text_secondary)
                    .wrap(true)
                    .into(),
                SiteLink {
                    key: "footer.attribution",
                    href: "https://fission.rs".into(),
                    identifier: "fission-attribution",
                }
                .into(),
            ],
            ..Default::default()
        })
        .identifier("site-footer")
        .into()
    }
}
