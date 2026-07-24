use fission::prelude::*;

pub struct ShowcaseGrid {
    children: Vec<Widget>,
    min_track_width: f32,
    single_column: bool,
}

impl ShowcaseGrid {
    pub fn new<T>(children: Vec<T>, min_track_width: f32) -> Self
    where
        T: Into<Widget>,
    {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            min_track_width,
            single_column: false,
        }
    }

    pub fn single_column<T>(children: Vec<T>) -> Self
    where
        T: Into<Widget>,
    {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            min_track_width: 0.0,
            single_column: true,
        }
    }
}

impl From<ShowcaseGrid> for Widget {
    fn from(grid: ShowcaseGrid) -> Self {
        let (_, view) = fission::build::current::<crate::state::GalleryState>();
        let spacing = &view.env().theme.tokens.spacing;

        let columns = if grid.single_column {
            vec![GridTrack::Fr(1.0)]
        } else {
            vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(grid.min_track_width),
                GridTrack::Fr(1.0),
            ))]
        };

        Grid {
            columns,
            rows: vec![GridTrack::Auto],
            column_gap: Some(spacing.l),
            row_gap: Some(spacing.l),
            children: grid.children,
            ..Default::default()
        }
        .into()
    }
}
