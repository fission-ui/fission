use crate::app::StoreState;
use crate::components::hero_copy::HeroCopy;
use crate::components::hero_spotlight::HeroSpotlight;
use crate::components::layout::HERO_SPOTLIGHT_WIDTH;
use fission::prelude::*;

fission_component! {
    #[derive(Clone)] pub Hero<StoreState> {}
    |_hero, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Container(Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(HERO_SPOTLIGHT_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: tokens.spacing.xl,
            row_gap: tokens.spacing.xl,
            children: [HeroCopy(), HeroSpotlight()],
        ))
        .padding_all(tokens.spacing.xl)
        .border(tokens.colors.primary.with_alpha(80), 1.0)
        .border_radius(tokens.radii.xxl)
        .bg(tokens.colors.surface_sunken)
}
