use fission::core::GlobalState;
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
use fission::prelude::DesktopApp;
use fission::widgets::{Column, Container, Text, Widget};

mod icon_gallery_list;
mod icon_gallery_row;
mod layout;

use icon_gallery_list::IconGalleryList;

#[derive(Default, Clone, Debug)]
pub struct State;

impl GlobalState for State {}

#[derive(Clone)]
pub struct IconsApp;

impl From<IconsApp> for Widget {
    fn from(_component: IconsApp) -> Self {
        let (_, view) = fission::build::current::<State>();
        let tokens = &view.env().theme.tokens;
        let total = fission::icons::material::all_icons().len();

        Container::new(Column {
            gap: Some(tokens.spacing.l),
            flex_grow: 1.0,
            children: fission::widgets![
                Text::new("Material Icons Gallery")
                    .size(tokens.typography.heading1_size)
                    .color(tokens.colors.heading),
                Text::new(format!("{total} icon variants"))
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary),
                IconGalleryList,
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .bg(tokens.colors.background)
        .flex_grow(1.0)
        .into()
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::<State, _>::new(IconsApp).run()
}
