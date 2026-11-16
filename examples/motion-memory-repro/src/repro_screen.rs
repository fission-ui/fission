use crate::layout::{REPRO_SCROLL_WIDTH, REPRO_SURFACE_HEIGHT};
use crate::repro_row::ReproRow;
use crate::scenario::ReproScenario;
use fission::prelude::*;

fission_component! {
    pub(crate) ReproScreen<()> {
        scenario: ReproScenario,
        rows: usize,
        row_height: f32,
        image_paths: Vec<String>,
        cache_images: bool,
    } |screen, _state, cx| {
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        Scroll(
            id: WidgetId::explicit("repro_scroll"),
            direction: FlexDirection::Column,
            width: REPRO_SCROLL_WIDTH,
            height: REPRO_SURFACE_HEIGHT,
            show_scrollbar: true,
            child: Column(
                gap: spacing.s,
                children: [
                    for index in 0..screen.rows {
                        ReproRow(
                            index,
                            screen.scenario,
                            screen.row_height,
                            screen.image_paths
                                .get(index % screen.image_paths.len().max(1))
                                .cloned(),
                            screen.cache_images,
                            key: index,
                        )
                    },
                ],
            ),
        )
}
