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
                PolicyCard {
                    title: "Full",
                    body: "FromTop + Fade + Scale",
                    active: cards.selected == MotionPolicy::Full,
                },
                PolicyCard {
                    title: "Reduced",
                    body: "Fade only, shorter duration",
                    active: cards.selected == MotionPolicy::Reduced,
                },
                PolicyCard {
                    title: "Disabled",
                    body: "Instant final state",
                    active: cards.selected == MotionPolicy::Disabled,
                },
            ],
        ))
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
