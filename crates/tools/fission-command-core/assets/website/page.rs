use crate::design::{message, CONTENT_WIDTH, NARROW_WIDTH};
use crate::footer::Footer;
use crate::navigation::{Navigation, SiteLink};
use fission::core::op::JustifyContent;
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

#[derive(Clone)]
pub struct SitePage(pub Page);

impl From<SitePage> for Widget {
    fn from(page: SitePage) -> Self {
        Responsive::new(PageLayout {
            page: page.0,
            narrow: false,
        })
        .case(ResponsiveCase::max_width(
            NARROW_WIDTH,
            PageLayout {
                page: page.0,
                narrow: true,
            },
        ))
        .into()
    }
}

#[derive(Clone)]
struct PageLayout {
    page: Page,
    narrow: bool,
}

impl From<PageLayout> for Widget {
    fn from(layout: PageLayout) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
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
        let introduction = Column {
            gap: Some(tokens.spacing.l),
            children: vec![
                Text::new(message(eyebrow))
                    .color(tokens.colors.primary)
                    .wrap(true)
                    .into(),
                SemanticsRegion::new(
                    Text::new(message(title))
                        .size(heading_size)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .wrap(true),
                )
                .identifier(if layout.narrow {
                    "site-heading-1:page-title-narrow"
                } else {
                    "site-heading-1:page-title-wide"
                })
                .into(),
                Text::new(message(body))
                    .size(tokens.typography.body_large_size)
                    .wrap(true)
                    .into(),
                SiteLink {
                    key: link,
                    href: layout.page.href(destination),
                    identifier: "page-next",
                }
                .into(),
                Text::new(message(note))
                    .color(tokens.colors.text_secondary)
                    .wrap(true)
                    .into(),
            ],
            ..Default::default()
        };
        let page = Container::new(Column {
            gap: Some(if layout.narrow {
                tokens.spacing.xl
            } else {
                tokens.spacing.xxxl
            }),
            children: vec![
                Navigation {
                    page: layout.page,
                    narrow: layout.narrow,
                }
                .into(),
                SemanticsRegion::new(introduction)
                    .identifier("site-main")
                    .into(),
                Footer.into(),
            ],
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .max_width(CONTENT_WIDTH)
        .padding_all(if layout.narrow {
            tokens.spacing.m
        } else {
            tokens.spacing.xxl
        });
        Container::new(Row {
            justify_content: JustifyContent::Center,
            children: vec![page.into()],
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .bg(tokens.colors.background)
        .into()
    }
}
