use super::policy_card::PolicyCard;
use crate::state::{AnimationGalleryState, MotionPolicy};
use crate::style::{BORDER, SURFACE};
use fission::prelude::*;

const POLICY_CARD_MIN_WIDTH: f32 = 168.0;

pub(super) struct PolicyCards {
    pub selected: MotionPolicy,
}

impl From<PolicyCards> for Widget {
    fn from(cards: PolicyCards) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(POLICY_CARD_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(tokens.spacing.s),
            row_gap: Some(tokens.spacing.s),
            children: widgets![
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
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
