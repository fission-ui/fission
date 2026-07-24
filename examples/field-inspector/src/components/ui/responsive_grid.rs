use crate::model::FieldInspectorState;
use fission::prelude::*;

const DEFAULT_ITEM_MIN_WIDTH: f32 = 176.0;

pub struct ResponsiveGrid {
    pub children: Vec<Widget>,
    pub item_min_width: f32,
}

impl ResponsiveGrid {
    pub fn new(children: Vec<Widget>) -> Self {
        Self {
            children,
            item_min_width: DEFAULT_ITEM_MIN_WIDTH,
        }
    }

    pub fn item_min_width(mut self, width: f32) -> Self {
        self.item_min_width = width;
        self
    }
}

impl From<ResponsiveGrid> for Widget {
    fn from(grid: ResponsiveGrid) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let gap = view.env().theme.tokens.spacing.s;

        Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(grid.item_min_width),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(gap),
            row_gap: Some(gap),
            children: grid.children,
            ..Default::default()
        }
        .into()
    }
}
