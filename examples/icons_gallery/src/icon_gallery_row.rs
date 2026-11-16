use crate::layout::{ICON_ROW_HEIGHT, ICON_SIZE, ROW_BORDER_WIDTH};
use crate::State;
use fission::op::AlignItems;
use fission::prelude::*;

fission_component! {
    pub IconGalleryRow<State> {
        index: usize,
        label: String,
        icon: fn() -> &'static str,
    } |item, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Row(
                gap: tokens.spacing.m,
                align_items: AlignItems::Center,
                children: [
                    Icon::svg((item.icon)())
                        .size(ICON_SIZE)
                        .color(tokens.colors.text_primary),
                    Text(item.label.clone())
                        .size(tokens.typography.font_size_sm)
                        .color(tokens.colors.text_secondary),
                ],
            ),
        )
        .height(ICON_ROW_HEIGHT)
        .padding_all(tokens.spacing.s)
        .bg(if item.index % 2 == 0 {
            tokens.colors.surface
        } else {
            tokens.colors.surface_sunken
        })
        .border(tokens.colors.divider, ROW_BORDER_WIDTH)
        .id(WidgetId::explicit(&format!(
            "icons-gallery.icon.{}",
            item.label,
        )))
}
