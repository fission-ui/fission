use fission::prelude::*;

use super::DROP_PANEL_MIN_WIDTH;
use crate::GalleryState;

const EXTERNAL_PANEL_MAX_WIDTH: f32 = 280.0;
const EXTERNAL_PANEL_MIN_HEIGHT: f32 = 86.0;

pub(super) struct ExternalFileDropPanel {
    pub hovered: bool,
}

impl From<ExternalFileDropPanel> for Widget {
    fn from(panel: ExternalFileDropPanel) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let mut children = widgets![Text::new(if panel.hovered {
            "Release to import"
        } else {
            "External files"
        })
        .size(tokens.typography.body_large_size)
        .weight(tokens.typography.font_weight_bold)
        .color(tokens.colors.text_primary),];

        if view.state().drag_external_files.is_empty() {
            children.push(
                Text::new("Drop files from Finder, Explorer, or the test driver here.")
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );
        } else {
            children.extend(
                view.state()
                    .drag_external_files
                    .iter()
                    .take(4)
                    .cloned()
                    .map(|file| {
                        Text::new(file)
                            .size(tokens.typography.body_medium_size)
                            .color(tokens.colors.text_primary)
                            .into()
                    }),
            );
        }

        Container::new(VStack {
            spacing: Some(tokens.spacing.xs),
            children,
        })
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
        .into()
    }
}
