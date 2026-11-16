use crate::design::{message, CONTENT_WIDTH, NARROW_WIDTH};
use crate::footer::Footer;
use crate::navigation::{Navigation, SiteLink};
use crate::state::WebsiteState;
use fission::op::JustifyContent;
use fission::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    About,
}

impl Page {
    pub fn href(self, destination: Self) -> String {
        #[cfg(target_arch = "wasm32")]
        {
            return match destination {
                Self::Home => "#/",
                Self::About => "#/about/",
            }
            .into();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            match (self, destination) {
                (Self::Home, Self::Home) | (Self::About, Self::About) => "./",
                (Self::Home, Self::About) => "about/",
                (Self::About, Self::Home) => "../",
            }
            .into()
        }
    }
}

fission_component! {
    #[derive(Clone)]
    pub SitePage<WebsiteState> {
        props { page: Page }
    }

    view(page, _state, _cx) =>
        Responsive::new(
            PageLayout::new(page.page, false).id(WidgetId::explicit("page-layout-wide")),
        )
            .case(ResponsiveCase::max_width(
                NARROW_WIDTH,
                PageLayout::new(page.page, true).id(WidgetId::explicit("page-layout-narrow")),
            ))
}

fission_component! {
    #[derive(Clone)]
    PageLayout<WebsiteState> {
        props {
            page: Page,
            narrow: bool,
        }
    }

    view(layout, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let (eyebrow, title, body, link, note, destination) = match layout.page {
            Page::Home => (
                "home.eyebrow",
                "home.title",
                "home.body",
                "home.link",
                "home.note",
                Page::About,
            ),
            Page::About => (
                "about.eyebrow",
                "about.title",
                "about.body",
                "about.link",
                "about.note",
                Page::Home,
            ),
        };
        let heading_size = if layout.narrow {
            tokens.typography.display_sm_size
        } else {
            tokens.typography.display_md_size
        };
        let heading_identifier = if layout.narrow {
            "site-heading-1:page-title-narrow"
        } else {
            "site-heading-1:page-title-wide"
        };
    } =>
    Container(
        Row(
            justify_content: JustifyContent::Center,
            children: [
                Container(
                    Column(
                        gap: if layout.narrow {
                            tokens.spacing.xl
                        } else {
                            tokens.spacing.xxxl
                        },
                        children: [
                            Navigation::new(layout.page, layout.narrow),
                            SemanticsRegion(
                                Column(
                                    gap: tokens.spacing.l,
                                    children: [
                                        Text::new(message(eyebrow))
                                            .color(tokens.colors.primary)
                                            .wrap(true),
                                        SemanticsRegion(
                                            Text::new(message(title))
                                                .size(heading_size)
                                                .weight(tokens.typography.font_weight_bold)
                                                .color(tokens.colors.heading)
                                                .wrap(true),
                                            identifier: Some(heading_identifier.to_string()),
                                        ),
                                        Text::new(message(body))
                                            .size(tokens.typography.body_large_size)
                                            .wrap(true),
                                        SiteLink::new(
                                            link,
                                            layout.page.href(destination),
                                            "page-next",
                                        ),
                                        Text::new(message(note))
                                            .color(tokens.colors.text_secondary)
                                            .wrap(true),
                                    ],
                                ),
                                identifier: Some("site-main".to_string()),
                            ),
                            Footer,
                        ],
                    ),
                )
                .width_length(Length::percent(100.0))
                .max_width(CONTENT_WIDTH)
                .padding_all(if layout.narrow {
                    tokens.spacing.m
                } else {
                    tokens.spacing.xxl
                }),
            ],
        ),
    )
    .width_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}
