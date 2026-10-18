use fission::prelude::*;

use super::DROP_PANEL_MIN_WIDTH;
use crate::GalleryState;

const EXTERNAL_PANEL_MAX_WIDTH: f32 = 280.0;
const EXTERNAL_PANEL_MIN_HEIGHT: f32 = 86.0;

fission_component! {
    pub(super) ExternalFileDropPanel<GalleryState> { hovered: bool }
    |panel, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(VStack(
            spacing: tokens.spacing.xs,
            children: [
                Text(if panel.hovered { "Release to import" } else { "External files" })
                    .size(tokens.typography.body_large_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                if state.drag_external_files.is_empty() {
                    Text("Drop files from Finder, Explorer, or the test driver here.")
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_secondary)
                },
                for file in state.drag_external_files.iter().take(4) {
                    Text(file.clone(), key: file.clone())
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_primary)
                },
            ],
        ))
        .width_length(Length::clamp(
            Length::points(DROP_PANEL_MIN_WIDTH),
            Length::percent(100.0),
            Length::points(EXTERNAL_PANEL_MAX_WIDTH),
        ))
        .min_height_length(Length::points(EXTERNAL_PANEL_MIN_HEIGHT))
        .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
        .border(
            if panel.hovered {
                tokens.colors.primary
            } else {
                tokens.colors.border
            },
            if panel.hovered { 2.0 } else { 1.0 },
        )
        .border_radius(tokens.radii.xl)
        .bg(if panel.hovered {
            tokens.colors.primary.with_alpha(32)
        } else {
            tokens.colors.background.with_alpha(28)
        })
}
