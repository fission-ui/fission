use crate::app::StoreState;
use crate::components::hero_copy::HeroCopy;
use crate::components::hero_spotlight::HeroSpotlight;
use crate::components::layout::HERO_SPOTLIGHT_WIDTH;
use crate::components::palette::{BLUE, HERO_SURFACE};
use fission::prelude::*;

#[derive(Clone)]
pub struct Hero;

impl From<Hero> for Widget {
    fn from(_component: Hero) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(HERO_SPOTLIGHT_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(tokens.spacing.xl),
            row_gap: Some(tokens.spacing.xl),
            children: widgets![HeroCopy, HeroSpotlight],
            ..Default::default()
        })
        .padding_all(tokens.spacing.xl)
        .border(BLUE.with_alpha(80), 1.0)
        .border_radius(tokens.radii.xxl)
        .bg(HERO_SURFACE)
        .into()
    }
}
