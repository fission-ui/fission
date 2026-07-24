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

pub(super) struct LaneDropzonePanel {
    pub title: &'static str,
    pub items: Vec<String>,
    pub state: LanePanelState,
    pub instance: &'static str,
}

impl From<LaneDropzonePanel> for Widget {
    fn from(panel: LaneDropzonePanel) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let hovered = matches!(panel.state, LanePanelState::Hovered);
        let mut children = widgets![
            HStack {
                spacing: Some(tokens.spacing.s),
                children: widgets![
                    Text::new(panel.title)
                        .size(tokens.typography.body_large_size)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.text_primary),
                    Tag {
                        label: panel.items.len().to_string(),
                        on_close: None,
                    },
                ],
            },
            Text::new(if hovered {
                "Release to drop here"
            } else {
                "Drag cards into this lane"
            })
            .size(tokens.typography.body_medium_size)
            .color(tokens.colors.text_secondary),
        ];
        children.extend(panel.items.into_iter().map(|label| {
            TaskCard {
                label,
                snap_to_grid: view.state().drag_snap_preview,
                instance: panel.instance,
            }
            .into()
        }));

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

        Container::new(VStack {
            spacing: Some(tokens.spacing.s),
            children,
        })
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
        .into()
    }
}
