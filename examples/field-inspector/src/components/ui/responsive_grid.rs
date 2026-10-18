use crate::model::FieldInspectorState;
use fission::prelude::*;

pub const DEFAULT_ITEM_MIN_WIDTH: f32 = 176.0;

fission_component! {
    pub ResponsiveGrid<FieldInspectorState> { children: Vec<Widget>, item_min_width: f32 }
    |grid, _state, cx| {
        let gap = cx.view.env().theme.tokens.spacing.s;
    } =>
        Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(grid.item_min_width),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: gap,
            row_gap: gap,
            children: grid.children,
        )
}
