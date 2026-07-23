use crate::layout::{ICON_ROW_HEIGHT, ICON_SIZE, ROW_BORDER_WIDTH};
use crate::State;
use fission::op::AlignItems;
use fission::prelude::*;
use fission::widgets::{Icon, Row};

pub struct IconGalleryRow {
    pub index: usize,
    pub label: String,
    pub icon: fn() -> &'static str,
}

impl From<IconGalleryRow> for Widget {
    fn from(item: IconGalleryRow) -> Self {
        let (_, view) = fission::build::current::<State>();
        let tokens = &view.env().theme.tokens;
        let row = Row {
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            children: widgets![
                Icon::svg((item.icon)())
                    .size(ICON_SIZE)
                    .color(tokens.colors.text_primary),
                Text::new(item.label)
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        };

        Container::new(row)
            .height(ICON_ROW_HEIGHT)
            .padding_all(tokens.spacing.s)
            .bg(if item.index % 2 == 0 {
                tokens.colors.surface
            } else {
                tokens.colors.surface_sunken
            })
            .border(tokens.colors.divider, ROW_BORDER_WIDTH)
            .into()
    }
}
