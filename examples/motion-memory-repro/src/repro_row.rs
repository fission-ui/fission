use crate::scenario::ReproScenario;
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct ReproRow {
    pub(crate) index: usize,
    pub(crate) scenario: ReproScenario,
    pub(crate) height: f32,
    pub(crate) image_path: Option<String>,
    pub(crate) cache_image: bool,
}

impl From<ReproRow> for Widget {
    fn from(row: ReproRow) -> Self {
        let (_ctx, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let accent = if row.index % 3 == 0 {
            REPRO_ACCENT_COOL
        } else {
            REPRO_ACCENT_WARM
        };

        let media: Widget = if let Some(path) = row.image_path {
            let size = row.height - tokens.spacing.m;
            let image = Image::file(path).size(size, size);
            if row.cache_image {
                image
                    .cache_size(size.ceil() as u32, size.ceil() as u32)
                    .into()
            } else {
                image.into()
            }
        } else {
            Container::new(Spacer {
                width: Some(REPRO_ACCENT_WIDTH),
                height: Some(row.height - tokens.spacing.m),
                ..Default::default()
            })
            .width(REPRO_ACCENT_WIDTH)
            .height(row.height - tokens.spacing.m)
            .bg(accent)
            .border_radius(tokens.radii.small)
            .into()
        };

        Container::new(Row {
            gap: Some(tokens.spacing.m),
            children: widgets![
                media,
                Column {
                    gap: Some(tokens.spacing.xs),
                    children: widgets![
                        Text::new(format!("{} / row {}", row.scenario.label(), row.index + 1))
                            .size(tokens.typography.body_medium_size),
                        Text::new("Repeated scroll content to reproduce retained renderer memory.")
                            .size(tokens.typography.font_size_xs),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .width(REPRO_ROW_WIDTH)
        .height(row.height)
        .padding([
            tokens.spacing.m,
            tokens.spacing.m,
            tokens.spacing.s,
            tokens.spacing.s,
        ])
        .bg(tokens.colors.surface)
        .border(REPRO_ROW_BORDER, 1.0)
        .border_radius(tokens.radii.large)
        .into()
    }
}
use crate::layout::{REPRO_ACCENT_WIDTH, REPRO_ROW_WIDTH};
use crate::palette::{REPRO_ACCENT_COOL, REPRO_ACCENT_WARM, REPRO_ROW_BORDER};
