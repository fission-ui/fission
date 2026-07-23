use fission::prelude::*;

use super::DROP_PANEL_MIN_WIDTH;
use crate::GalleryState;

const LOG_MAX_WIDTH: f32 = 280.0;
const LOG_MIN_HEIGHT: f32 = 116.0;

pub(super) struct DragLog;

impl From<DragLog> for Widget {
    fn from(_log: DragLog) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let rows = if view.state().drag_log.is_empty() {
            widgets![Text::new("Drag log is empty.")
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary),]
        } else {
            view.state()
                .drag_log
                .iter()
                .cloned()
                .map(|entry| {
                    Text::new(entry)
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_primary)
                        .into()
                })
                .collect()
        };

        Container::new(VStack {
            spacing: Some(tokens.spacing.xs),
            children: widgets![
                Text::new("Interaction log")
                    .size(tokens.typography.body_large_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                VStack {
                    spacing: Some(tokens.spacing.xs),
                    children: rows,
                },
            ],
        })
        .width_length(Length::clamp(
            Length::points(DROP_PANEL_MIN_WIDTH),
            Length::percent(100.0),
            Length::points(LOG_MAX_WIDTH),
        ))
        .min_height_length(Length::points(LOG_MIN_HEIGHT))
        .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xl)
        .into()
    }
}
