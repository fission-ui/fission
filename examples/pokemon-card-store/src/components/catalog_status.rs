use crate::app::StoreState;
use crate::data::{CatalogResponse, StoreError};
use fission::prelude::*;

pub struct CatalogStatus {
    pub snapshot: AsyncSnapshot<CatalogResponse, StoreError>,
}

impl From<CatalogStatus> for Widget {
    fn from(status: CatalogStatus) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let (title, detail, accent) = if let Some(error) = status.snapshot.error() {
            (
                "Catalogue unavailable",
                error.message.as_str(),
                tokens.colors.error,
            )
        } else {
            (
                "Loading cards",
                "The server route declares a catalogue job and renders the completed state after the job drains.",
                tokens.colors.primary,
            )
        };

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new(title)
                    .size(typography.font_size_xl)
                    .line_height(typography.font_size_xl * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                Text::new(detail)
                    .size(typography.body_medium_size)
                    .line_height(typography.body_medium_size * typography.line_height_normal)
                    .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .border(accent.with_alpha(120), 1.0)
        .border_radius(tokens.radii.xxl)
        .bg(tokens.colors.surface)
        .into()
    }
}
