use super::color;
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;

pub struct StatusPill {
    pub label: String,
    pub state: CapabilityState,
}

impl StatusPill {
    pub fn new(label: impl Into<String>, state: CapabilityState) -> Self {
        Self {
            label: label.into(),
            state,
        }
    }
}

impl From<StatusPill> for Widget {
    fn from(pill: StatusPill) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let (background, foreground) = match pill.state {
            CapabilityState::Idle => (tokens.colors.surface, tokens.colors.text_secondary),
            CapabilityState::Pending => (color(254, 243, 199), color(146, 64, 14)),
            CapabilityState::Ready => (color(219, 234, 254), color(29, 78, 216)),
            CapabilityState::Complete => (color(220, 252, 231), color(21, 128, 61)),
            CapabilityState::Unavailable => (color(229, 231, 235), color(75, 85, 99)),
            CapabilityState::Warning => (color(254, 249, 195), color(133, 77, 14)),
            CapabilityState::Error => (color(254, 226, 226), color(185, 28, 28)),
        };
        let typography = &tokens.typography;

        Container::new(
            Text::new(pill.label)
                .size(typography.font_size_xs)
                .line_height(typography.font_size_xs * typography.line_height_snug)
                .weight(typography.font_weight_bold)
                .wrap(false)
                .color(foreground),
        )
        .bg(background)
        .border_radius(tokens.radii.full)
        .padding([
            tokens.spacing.s,
            tokens.spacing.s,
            tokens.spacing.xs,
            tokens.spacing.xs,
        ])
        .into()
    }
}
