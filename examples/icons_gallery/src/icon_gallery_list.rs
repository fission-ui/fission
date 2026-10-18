use crate::icon_gallery_row::IconGalleryRow;
use crate::layout::ICON_ROW_HEIGHT;
use crate::model::{IconEntry, State};
use fission::prelude::*;

fission_component! {
    pub IconGalleryList<State> { entries: Vec<IconEntry> } |list, _state, _cx| {} =>
        if list.entries.is_empty() {
            EmptyState(
                title: "No icons match".to_string(),
                description: "Try a shorter name, or pick a different category.".to_string(),
            )
        } else {
            LazyColumn(
                item_height: ICON_ROW_HEIGHT,
                children: [
                    for (index, (category, name, variant, icon)) in
                        list.entries.iter().copied().enumerate()
                    {
                        IconGalleryRow(
                            index,
                            format!("{category}/{name}/{variant}"),
                            icon,
                            key: format!("{category}/{name}/{variant}"),
                        )
                    },
                ],
            )
        }
}
