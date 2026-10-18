#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
use fission::prelude::DesktopApp;
use fission::prelude::*;

mod icon_gallery_controls;
mod icon_gallery_list;
mod icon_gallery_row;
mod layout;
mod model;

use icon_gallery_controls::IconGalleryControls;
use icon_gallery_list::IconGalleryList;

pub use model::{
    filter_icons, on_category_selected, on_search_changed, CategorySelected, IconEntry,
    IconResults, SearchChanged, State,
};

fission_component! {
    pub IconsApp<State>
    |_app, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let results = filter_icons(state);
        let shown = results.visible.len();
    } =>
    Container(
        Column(
            gap: tokens.spacing.l,
            flex_grow: 1.0_f32,
            children: [
                Text("Material Icons Gallery")
                    .size(tokens.typography.heading1_size)
                    .color(tokens.colors.heading),
                IconGalleryControls {
                    categories: results.categories,
                    shown,
                    total: results.total,
                },
                IconGalleryList {
                    entries: results.visible,
                },
            ],
        ),
    )
    .padding_all(tokens.spacing.l)
    .bg(tokens.colors.background)
    .flex_grow(1.0)
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::<State, _>::new(IconsApp).run()
}
