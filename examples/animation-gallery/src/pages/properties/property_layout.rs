use super::{property_info_panel::PropertyInfoPanel, property_workspace::PropertyWorkspace};
use crate::state::AnimationGalleryState;
use fission::prelude::*;

const PANEL_MIN_WIDTH: f32 = 280.0;

fission_component! {
    pub(super) PropertyLayout<AnimationGalleryState> { property: super::PropertyCase }
    |layout, _state, cx| {
        let gap = cx.view.env().theme.tokens.spacing.s;
    } =>
        Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(PANEL_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: gap,
            row_gap: gap,
            children: [
                PropertyWorkspace(layout.property.clone()),
                PropertyInfoPanel(layout.property),
            ],
        )
}
