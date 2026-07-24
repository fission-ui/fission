use fission::prelude::*;

use super::{drag_ended, drag_started, DragEnded, DragStarted, DRAG_GRID_SIZE};
use crate::GalleryState;

#[derive(Clone)]
pub(super) struct TaskCard {
    pub label: String,
    pub snap_to_grid: bool,
    pub instance: &'static str,
}

impl From<TaskCard> for Widget {
    fn from(card: TaskCard) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let identifier = format!("gallery.drag.card.{}.{}", card.instance, card.label);

        Draggable {
            id: Some(WidgetId::explicit(&identifier)),
            semantics_identifier: Some(identifier),
            payload: card.label.as_bytes().to_vec(),
            child: Container::new(HStack {
                spacing: Some(tokens.spacing.s),
                children: widgets![
                    Icon::svg(fission::icons::material::action::drag_indicator::round())
                        .size(tokens.typography.body_large_size)
                        .color(tokens.colors.text_secondary),
                    Text::new(card.label.clone())
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_primary),
                ],
            })
            .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.large)
            .bg(tokens.colors.surface)
            .into(),
            preview: Some(
                Container::new(HStack {
                    spacing: Some(tokens.spacing.xs),
                    children: widgets![
                        Icon::svg(fission::icons::material::action::drag_indicator::round())
                            .size(tokens.typography.body_medium_size)
                            .color(tokens.colors.primary),
                        Text::new(card.label.clone())
                            .size(tokens.typography.body_medium_size)
                            .color(tokens.colors.text_primary),
                    ],
                })
                .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
                .border(tokens.colors.primary, 1.0)
                .border_radius(tokens.radii.large)
                .bg(tokens.colors.surface.with_alpha(235))
                .into(),
            ),
            preview_options: DragPreviewOptions {
                snap_grid: card.snap_to_grid.then_some(DRAG_GRID_SIZE),
                ..Default::default()
            },
            on_drag_start: Some(
                ctx.bind(DragStarted(card.label.clone()), reduce_with!(drag_started)),
            ),
            on_drag_end: Some(ctx.bind(DragEnded(card.label), reduce_with!(drag_ended))),
        }
        .into()
    }
}
