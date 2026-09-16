use crate::icon_gallery_row::IconGalleryRow;
use crate::layout::ICON_ROW_HEIGHT;
use crate::model::IconEntry;
use fission::prelude::*;
use fission::widgets::LazyColumn;

pub struct IconGalleryList {
    /// The icons the current filters leave visible, in reflection order.
    pub entries: Vec<IconEntry>,
}

impl From<IconGalleryList> for Widget {
    fn from(list: IconGalleryList) -> Self {
        if list.entries.is_empty() {
            // A filter that matches nothing must say so; an empty list reads as
            // a broken screen.
            return EmptyState {
                icon: None,
                title: "No icons match".to_string(),
                description: Some(
                    "Try a shorter name, or pick a different category.".to_string(),
                ),
                action: None,
            }
            .into();
        }

        let children = list
            .entries
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
