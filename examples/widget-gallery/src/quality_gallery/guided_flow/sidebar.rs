use super::palette::{rgba, BLUE, INK, LINE};
use fission::prelude::*;

pub(super) struct GuidedSidebar;
impl From<GuidedSidebar> for Widget {
    fn from(_sidebar: GuidedSidebar) -> Self {
        Container::new(Column {
            gap: Some(12.0),
            align_items: fission::op::AlignItems::Center,
            children: widgets![
                Container::new(
                    Icon::svg(material::hardware::toys::regular())
                        .size(28.0)
                        .color(BLUE),
                )
                .size(44.0, 44.0)
                .align_child(BoxAlignment::Center),
                Spacer {
                    height: Some(16.0),
                    ..Default::default()
                },
                GuidedNavIcon {
                    destination: NavDestination::Home,
                    active: false
                },
                GuidedNavIcon {
                    destination: NavDestination::Files,
                    active: false
                },
                GuidedNavIcon {
                    destination: NavDestination::Mail,
                    active: false
                },
                GuidedNavIcon {
                    destination: NavDestination::Links,
                    active: false
                },
                GuidedNavIcon {
                    destination: NavDestination::Templates,
                    active: true
                },
                GuidedNavIcon {
                    destination: NavDestination::Guides,
                    active: false
                },
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Divider::default(),
                GuidedNavIcon {
                    destination: NavDestination::Layers,
                    active: false
                },
                GuidedNavIcon {
                    destination: NavDestination::Settings,
                    active: false
                },
                Container::new(Avatar {
                    name: Some("Alex Stone".into()),
                    size: Some(34.0),
                    ..Default::default()
                })
                .border(Color::WHITE, 2.0)
                .border_radius(17.0),
            ],
            ..Default::default()
        })
        .width(80.0)
        .padding([10.0, 10.0, 18.0, 14.0])
        .border(LINE, 1.0)
        .into()
    }
}

pub(super) struct GuidedNavIcon {
    destination: NavDestination,
    active: bool,
}
impl From<GuidedNavIcon> for Widget {
    fn from(item: GuidedNavIcon) -> Self {
        let source = item.destination.icon();
        Container::new(
            Icon::svg(source)
                .size(21.0)
                .color(if item.active { BLUE } else { INK }),
        )
        .size(46.0, 42.0)
        .align_child(BoxAlignment::Center)
        .bg(if item.active {
            rgba(238, 242, 255, 255)
        } else {
            Color::TRANSPARENT
        })
        .border_radius(7.0)
        .into()
    }
}

#[derive(Clone, Copy)]
pub(super) enum NavDestination {
    Home,
    Files,
    Mail,
    Links,
    Templates,
    Guides,
    Layers,
    Settings,
}

impl NavDestination {
    fn icon(self) -> &'static str {
        match self {
            NavDestination::Home => material::action::home::regular(),
            NavDestination::Files => material::file::folder::regular(),
            NavDestination::Mail => material::communication::email::regular(),
            NavDestination::Links => material::content::link::regular(),
            NavDestination::Templates => material::action::description::regular(),
            NavDestination::Guides => material::maps::menu_book::regular(),
            NavDestination::Layers => material::maps::layers::regular(),
            NavDestination::Settings => material::action::settings::regular(),
        }
    }
}
