use crate::app::StoreState;
use crate::components::card_tile::CardTile;
use crate::components::catalog_section_title::CatalogSectionTitle;
use crate::components::catalog_status::CatalogStatus;
use crate::components::layout::CARD_GRID_MIN_WIDTH;
use crate::data::{CatalogResponse, StoreError};
use fission::prelude::*;

fission_component! {
    #[derive(Clone)] pub CardGrid<StoreState> {
        snapshot: AsyncSnapshot<CatalogResponse, StoreError>,
    } |component, _state, cx| {
        let catalog = component.snapshot.data();
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        if let Some(catalog) = catalog {
            Column(
                gap: spacing.m,
                children: [
                    CatalogSectionTitle(),
                    Grid(
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(CARD_GRID_MIN_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: spacing.m,
                    row_gap: spacing.m,
                    children: [
                        for card in catalog
                            .cards
                            .iter()
                            .filter_map(|summary| crate::data::card_by_slug(&summary.slug))
                            .cloned()
                        {
                            CardTile(card.clone(), key: card.slug)
                        },
                    ],
                ),
            ],
        )
        } else {
            CatalogStatus(component.snapshot.clone())
        }
}
