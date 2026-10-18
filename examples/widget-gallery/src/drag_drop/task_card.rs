use fission::prelude::*;

use super::{drag_ended, drag_started, DragEnded, DragStarted, DRAG_GRID_SIZE};
use crate::GalleryState;

fission_component! {
    #[derive(Clone)] pub(super) TaskCard<GalleryState> {
        label: String,
        snap_to_grid: bool,
        instance: &'static str,
    }
    |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let identifier = format!("gallery.drag.card.{}.{}", card.instance, card.label);

    } =>
        Draggable(
            id: WidgetId::explicit(&identifier),
            semantics_identifier: identifier,
            payload: card.label.as_bytes().to_vec(),
            child: Container(HStack(
                spacing: tokens.spacing.s,
                children: [
                    Icon::svg(fission::icons::material::action::drag_indicator::round())
                        .size(tokens.typography.body_large_size)
                        .color(tokens.colors.text_secondary),
                    Text(card.label.clone())
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_primary),
                ],
            ))
            .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.large)
            .bg(tokens.colors.surface)
            ,
            preview:
                Container(HStack(
                    spacing: tokens.spacing.xs,
                    children: [
                        Icon::svg(fission::icons::material::action::drag_indicator::round())
                            .size(tokens.typography.body_medium_size)
                            .color(tokens.colors.primary),
                        Text(card.label.clone())
                            .size(tokens.typography.body_medium_size)
                            .color(tokens.colors.text_primary),
                    ],
                ))
                .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
                .border(tokens.colors.primary, 1.0)
                .border_radius(tokens.radii.large)
                .bg(tokens.colors.surface.with_alpha(235)),
            preview_options: DragPreviewOptions {
                snap_grid: card.snap_to_grid.then_some(DRAG_GRID_SIZE),
                ..Default::default()
            },
            on_drag_start: cx.build.bind(DragStarted(card.label.clone()), reduce_with!(drag_started)),
            on_drag_end: cx.build.bind(
                DragEnded(card.label.clone()),
                reduce_with!(drag_ended),
            ),
        )
}
