use super::policy_card::PolicyCard;
use crate::state::{AnimationGalleryState, MotionPolicy};
use crate::style;
use fission::prelude::*;

const POLICY_CARD_MIN_WIDTH: f32 = 168.0;

fission_component! {
    pub(super) PolicyCards<AnimationGalleryState> { selected: MotionPolicy }
    |cards, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(POLICY_CARD_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: tokens.spacing.s,
            row_gap: tokens.spacing.s,
            children: [
                PolicyCard(
                    "Full",
                    "FromTop + Fade + Scale",
                    cards.selected == MotionPolicy::Full,
                    key: "full",
                ),
                PolicyCard(
                    "Reduced",
                    "Fade only, shorter duration",
                    cards.selected == MotionPolicy::Reduced,
                    key: "reduced",
                ),
                PolicyCard(
                    "Disabled",
                    "Instant final state",
                    cards.selected == MotionPolicy::Disabled,
                    key: "disabled",
                ),
            ],
        ))
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
