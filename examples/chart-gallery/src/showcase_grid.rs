use crate::state::GalleryState;
use fission::prelude::*;

fission_component! {
    #[derive(Default)]
    pub ShowcaseGrid<GalleryState> { children: Vec<Widget>, min_track_width: f32, single_column: bool }
    |grid, _state, cx| {
        let spacing = &cx.view.env().theme.tokens.spacing;
        let columns = if grid.single_column {
            vec![GridTrack::Fr(1.0)]
        } else {
            vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(grid.min_track_width),
                GridTrack::Fr(1.0),
            ))]
        };
    } =>
        Grid(
            columns: columns,
            rows: vec![GridTrack::Auto],
            column_gap: spacing.l,
            row_gap: spacing.l,
            children: grid.children,
        )
}

impl ShowcaseGrid {
    pub fn single_column<T>(children: Vec<T>) -> Self
    where
        T: Into<Widget>,
    {
        Self::new(children.into_iter().map(Into::into).collect(), 0.0, true)
    }
}
