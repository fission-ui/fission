use super::palette::{rgba, BLUE, INK, LINE};
use fission::prelude::*;

fission_component! {
    pub(super) GuidedSidebar<()> {}
    |_sidebar, _state, _cx| {} => Container(Column(
            gap: 12.0_f32,
            align_items: fission::op::AlignItems::Center,
            children: [
                Container(
                    Icon::svg(material::hardware::toys::regular())
                        .size(28.0)
                        .color(BLUE),
                )
                .size(44.0, 44.0)
                .align_child(BoxAlignment::Center),
                Spacer(height: 16.0_f32),
                GuidedNavIcon(NavDestination::Home, false),
                GuidedNavIcon(NavDestination::Files, false),
                GuidedNavIcon(NavDestination::Mail, false),
                GuidedNavIcon(NavDestination::Links, false),
                GuidedNavIcon(NavDestination::Templates, true),
                GuidedNavIcon(NavDestination::Guides, false),
                Spacer(flex_grow: 1.0_f32),
                Divider::default(),
                GuidedNavIcon(NavDestination::Layers, false),
                GuidedNavIcon(NavDestination::Settings, false),
                Container(Avatar(name: "Alex Stone".to_string(), size: 34.0_f32))
                .border(Color::WHITE, 2.0)
                .border_radius(17.0),
            ],
        ))
        .width(80.0)
        .padding([10.0, 10.0, 18.0, 14.0])
        .border(LINE, 1.0)
}

fission_component! {
    pub(super) GuidedNavIcon<()> { destination: NavDestination, active: bool }
    |item, _state, _cx| {
        let source = item.destination.icon();
    } => Container(
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
