use fission::prelude::*;

pub(crate) struct GalleryHeader;

impl From<GalleryHeader> for Widget {
    fn from(_header: GalleryHeader) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new("Fission Widget Gallery")
                    .size(typography.heading1_size)
                    .line_height(typography.heading1_size * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                Text::new(
                    "Interactive examples of Fission's built-in widgets and input behavior.",
                )
                .size(typography.body_large_size)
                .line_height(typography.body_large_size * typography.line_height_normal)
                .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        }
        .into()
    }
}
