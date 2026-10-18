use crate::layout::{REPRO_ACCENT_WIDTH, REPRO_ROW_WIDTH};
use crate::palette::{REPRO_ACCENT_COOL, REPRO_ACCENT_WARM, REPRO_ROW_BORDER};
use crate::scenario::ReproScenario;
use fission::prelude::*;

fission_component! {
    pub(crate) ReproRow<()> {
        index: usize,
        scenario: ReproScenario,
        height: f32,
        image_path: Option<String>,
        cache_image: bool,
    } |row, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let accent = if row.index % 3 == 0 {
            REPRO_ACCENT_COOL
        } else {
            REPRO_ACCENT_WARM
        };
        let media_size = row.height - tokens.spacing.m;
    } =>
        Container(
            Row(
                gap: tokens.spacing.m,
                children: [
                    if let Some(path) = row.image_path.clone() {
                        if row.cache_image {
                            Image::file(path)
                                .size(media_size, media_size)
                                .cache_size(media_size.ceil() as u32, media_size.ceil() as u32)
                        } else {
                            Image::file(path).size(media_size, media_size)
                        }
                    } else {
                        Container(
                            Spacer(width: REPRO_ACCENT_WIDTH, height: media_size),
                        )
                        .width(REPRO_ACCENT_WIDTH)
                        .height(media_size)
                        .bg(accent)
                        .border_radius(tokens.radii.small)
                    },
                    Column(
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        gap: tokens.spacing.xs,
                        children: [
                            Text(format!(
                                "{} / row {}",
                                row.scenario.label(),
                                row.index + 1,
                            ))
                            .size(tokens.typography.body_medium_size),
                            Text("Repeated scroll content to reproduce retained renderer memory.")
                                .size(tokens.typography.font_size_xs),
                        ],
                    ),
                ],
            ),
        )
        .width(REPRO_ROW_WIDTH)
        .min_height(row.height)
        .flex_shrink(0.0)
        .padding([
            tokens.spacing.m,
            tokens.spacing.m,
            tokens.spacing.s,
            tokens.spacing.s,
        ])
        .bg(tokens.colors.surface)
        .border(REPRO_ROW_BORDER, 1.0)
        .border_radius(tokens.radii.large)
}
