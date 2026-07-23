use super::{property_info_panel::PropertyInfoPanel, property_workspace::PropertyWorkspace};
use crate::state::AnimationGalleryState;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

const PANEL_MIN_WIDTH: f32 = 280.0;

pub(super) struct PropertyLayout<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub property: &'a super::PropertyCase,
}

impl From<PropertyLayout<'_>> for Widget {
    fn from(layout: PropertyLayout<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let gap = view.env().theme.tokens.spacing.s;

        Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(PANEL_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(gap),
            row_gap: Some(gap),
            children: widgets![
                PropertyWorkspace {
                    ctx: layout.ctx,
                    state: layout.state,
                    property: layout.property,
                },
                PropertyInfoPanel {
                    property: layout.property,
                    state: layout.state,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
