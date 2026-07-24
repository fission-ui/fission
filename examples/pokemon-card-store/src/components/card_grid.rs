use crate::app::StoreState;
use crate::components::card_tile::CardTile;
use crate::components::catalog_section_title::CatalogSectionTitle;
use crate::components::catalog_status::CatalogStatus;
use crate::components::layout::CARD_GRID_MIN_WIDTH;
use crate::data::{CatalogResponse, StoreError};
use fission::prelude::*;

#[derive(Clone)]
pub struct CardGrid {
    pub snapshot: AsyncSnapshot<CatalogResponse, StoreError>,
}

impl From<CardGrid> for Widget {
    fn from(component: CardGrid) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let Some(catalog) = component.snapshot.data() else {
            return CatalogStatus {
                snapshot: component.snapshot,
            }
            .into();
        };
        let children = catalog
            .cards
            .iter()
            .filter_map(|summary| crate::data::card_by_slug(&summary.slug))
            .cloned()
            .map(|card| CardTile { card }.into())
            .collect();
        let spacing = &view.env().theme.tokens.spacing;

        Column {
            gap: Some(spacing.m),
            children: widgets![
                CatalogSectionTitle,
                Grid {
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(CARD_GRID_MIN_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: Some(spacing.m),
                    row_gap: Some(spacing.m),
                    children,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
