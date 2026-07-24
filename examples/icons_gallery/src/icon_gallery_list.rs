use crate::icon_gallery_row::IconGalleryRow;
use crate::layout::ICON_ROW_HEIGHT;
use fission::prelude::*;
use fission::widgets::LazyColumn;

pub struct IconGalleryList;

impl From<IconGalleryList> for Widget {
    fn from(_list: IconGalleryList) -> Self {
        let children = fission::icons::material::all_icons()
            .into_iter()
            .enumerate()
            .map(|(index, (category, name, variant, icon))| {
                IconGalleryRow {
                    index,
                    label: format!("{category}/{name}/{variant}"),
                    icon,
                }
                .into()
            })
            .collect();

        LazyColumn {
            id: None,
            children,
            item_height: ICON_ROW_HEIGHT,
        }
        .into()
    }
}
