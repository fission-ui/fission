use crate::repro_row::ReproRow;
use crate::scenario::ReproScenario;
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct ReproScreen {
    pub(crate) scenario: ReproScenario,
    pub(crate) rows: usize,
    pub(crate) row_height: f32,
    pub(crate) image_paths: Vec<String>,
    pub(crate) cache_images: bool,
}

impl From<ReproScreen> for Widget {
    fn from(screen: ReproScreen) -> Self {
        let (_ctx, view) = fission::build::current::<()>();
        let spacing = &view.env().theme.tokens.spacing;
        Scroll {
            id: Some(WidgetId::explicit("repro_scroll")),
            child: Some(
                Column {
                    gap: Some(spacing.s),
                    children: (0..screen.rows)
                        .map(|index| {
                            ReproRow {
                                index,
                                scenario: screen.scenario,
                                height: screen.row_height,
                                image_path: screen
                                    .image_paths
                                    .get(index % screen.image_paths.len().max(1))
                                    .cloned(),
                                cache_image: screen.cache_images,
                            }
                            .into()
                        })
                        .collect(),
                    ..Default::default()
                }
                .into(),
            ),
            direction: FlexDirection::Column,
            width: Some(REPRO_SCROLL_WIDTH),
            height: Some(REPRO_SURFACE_HEIGHT),
            show_scrollbar: true,
            ..Default::default()
        }
        .into()
    }
}
use crate::layout::{REPRO_SCROLL_WIDTH, REPRO_SURFACE_HEIGHT};
