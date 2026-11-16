use crate::design::message;
use crate::page::Page;
use crate::state::WebsiteState;
use fission::op::{AlignItems, JustifyContent};
use fission::prelude::*;

fission_component! {
    #[derive(Clone)]
    pub SiteLink<WebsiteState> {
        props {
            key: &'static str,
            href: String,
            identifier: &'static str,
        }
    }

    view(link, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        Pressable(
            Text::new(message(link.key))
                .color(tokens.colors.text_link)
                .underline(true),
        )
        .href(link.href.clone())
        .on_press(
            NavigationRequested::new(NavigationCommand::Open(Hyperlink::new(
                link.href.clone(),
            )))
            .into(),
        )
        .layout(BoxStyle::default().padding_all(Length::points(tokens.spacing.ms))),
        identifier: Some(link.identifier.to_string()),
    )
}

fission_component! {
    #[derive(Clone)]
    pub SiteNavigationLinks<WebsiteState> {
        props { page: Page }
    }

    view(links, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            gap: tokens.spacing.s,
            children: [
                SiteLink::new("nav.home", links.page.href(Page::Home), "nav-home"),
                SiteLink::new("nav.about", links.page.href(Page::About), "nav-about"),
            ],
        )
}

fission_component! {
    #[derive(Clone)]
    pub Navigation<WebsiteState> {
        props {
            page: Page,
            narrow: bool,
        }
    }

    view(nav, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let brand = Text::new(message("brand"))
            .size(tokens.typography.heading_size)
            .weight(tokens.typography.font_weight_bold);
    } =>
    SemanticsRegion(
        if nav.narrow {
            Column(
                gap: tokens.spacing.s,
                children: [brand, SiteNavigationLinks::new(nav.page)],
            )
        } else {
            Row(
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                children: [brand, SiteNavigationLinks::new(nav.page)],
            )
        },
        identifier: Some("site-navigation".to_string()),
        role: Role::Group,
        label: Some(cx.view.env().tr("nav.label")),
    )
}
