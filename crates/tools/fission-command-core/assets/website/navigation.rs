use crate::design::message;
use crate::page::Page;
use fission::core::op::{AlignItems, JustifyContent};
use fission::prelude::*;

#[derive(Clone)]
pub struct SiteLink {
    pub key: &'static str,
    pub href: String,
    pub identifier: &'static str,
}

impl From<SiteLink> for Widget {
    fn from(link: SiteLink) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        SemanticsRegion::new(
            Pressable::new(
                Text::new(message(link.key))
                    .color(tokens.colors.text_link)
                    .underline(true),
            )
            .href(link.href.clone())
            .on_press(
                NavigationRequested::new(NavigationCommand::Open(Hyperlink::new(link.href))).into(),
            )
            .layout(BoxStyle::default().padding_all(Length::points(tokens.spacing.ms))),
        )
        .identifier(link.identifier)
        .into()
    }
}

#[derive(Clone)]
pub struct Navigation {
    pub page: Page,
    pub narrow: bool,
}

impl From<Navigation> for Widget {
    fn from(nav: Navigation) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let brand = Text::new(message("brand"))
            .size(tokens.typography.heading_size)
            .weight(tokens.typography.font_weight_bold)
            .into();
        let links: Widget = Row {
            gap: Some(tokens.spacing.s),
            children: vec![
                SiteLink {
                    key: "nav.home",
                    href: nav.page.href(Page::Home),
                    identifier: "nav-home",
                }
                .into(),
                SiteLink {
                    key: "nav.about",
                    href: nav.page.href(Page::About),
                    identifier: "nav-about",
                }
                .into(),
            ],
            ..Default::default()
        }
        .into();
        let content: Widget = if nav.narrow {
            Column {
                gap: Some(tokens.spacing.s),
                children: vec![brand, links],
                ..Default::default()
            }
            .into()
        } else {
            Row {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                children: vec![brand, links],
                ..Default::default()
            }
            .into()
        };
        SemanticsRegion::new(content)
            .identifier("site-navigation")
            .role(Role::Group)
            .label(
                view.env()
                    .i18n
                    .get(&view.env().locale, "nav.label")
                    .unwrap_or(""),
            )
            .into()
    }
}
