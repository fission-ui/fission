use fission::prelude::*;

use super::task_card::TaskCard;
use super::DROP_PANEL_MIN_WIDTH;
use crate::GalleryState;

const DROP_PANEL_MAX_WIDTH: f32 = 260.0;
const DROP_PANEL_MIN_HEIGHT: f32 = 190.0;

#[derive(Clone, Copy)]
pub(super) enum LanePanelState {
    Idle,
    Active,
    Hovered,
}

fission_component! {
    pub(super) LaneDropzonePanel<GalleryState> {
        title: String,
        items: Vec<String>,
        state: LanePanelState,
        instance: &'static str,
    }
    |panel, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let hovered = matches!(panel.state, LanePanelState::Hovered);
        let (border, border_width, background) = match panel.state {
            LanePanelState::Idle => (
                tokens.colors.border,
                1.0,
                tokens.colors.background.with_alpha(35),
            ),
            LanePanelState::Active => (
                tokens.colors.primary.with_alpha(150),
                1.0,
                tokens.colors.primary.with_alpha(18),
            ),
            LanePanelState::Hovered => (
                tokens.colors.primary,
                2.0,
                tokens.colors.primary.with_alpha(36),
            ),
        };

    } =>
        Container(VStack(
            spacing: tokens.spacing.s,
            children: [
                HStack(
                    spacing: tokens.spacing.s,
                    children: [
                        Text(panel.title)
                            .size(tokens.typography.body_large_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.text_primary),
                        Tag(label: panel.items.len().to_string(), selected: false),
                    ],
                ),
                Text(if hovered { "Release to drop here" } else { "Drag cards into this lane" })
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary),
                for label in panel.items.iter().cloned() {
                    TaskCard(label.clone(), state.drag_snap_preview, panel.instance, key: label)
                },
            ],
        ))
        .width_length(Length::clamp(
            Length::points(DROP_PANEL_MIN_WIDTH),
            Length::percent(100.0),
            Length::points(DROP_PANEL_MAX_WIDTH),
        ))
        .min_height_length(Length::points(DROP_PANEL_MIN_HEIGHT))
        .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
        .border(border, border_width)
        .border_radius(tokens.radii.xl)
        .bg(background)
}
