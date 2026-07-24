use crate::app::StoreState;
use crate::components::palette::{GREEN, TEXT_MUTED, TEXT_PRIMARY};
use fission::prelude::*;

pub struct CatalogSectionTitle;

impl From<CatalogSectionTitle> for Widget {
    fn from(_: CatalogSectionTitle) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Row {
            gap: Some(tokens.spacing.m),
            wrap: ir_op::FlexWrap::Wrap,
            children: widgets![
                Column {
                    gap: Some(tokens.spacing.xs),
                    flex_grow: 1.0,
                    children: widgets![
                        Text::new("Available cards")
                            .size(typography.heading_size)
                            .line_height(
                                typography.heading_size * typography.line_height_heading
                            )
                            .weight(typography.font_weight_bold)
                            .color(TEXT_PRIMARY),
                        Text::new("Generated as normal Fission widgets, rendered to HTML by the server shell, and combined with session cart state.")
                            .size(typography.body_medium_size)
                            .line_height(
                                typography.body_medium_size
                                    * typography.line_height_normal
                            )
                            .color(TEXT_MUTED),
                    ],
                    ..Default::default()
                },
                Container::new(
                    Text::new("Cart updates persist through the server session")
                        .size(typography.font_size_sm)
                        .line_height(
                            typography.font_size_sm * typography.line_height_snug
                        )
                        .weight(typography.font_weight_semibold)
                        .color(GREEN.with_alpha(230)),
                )
                .padding([
                    tokens.spacing.s,
                    tokens.spacing.s,
                    tokens.spacing.xs,
                    tokens.spacing.xs,
                ])
                .border(GREEN.with_alpha(120), 1.0)
                .border_radius(tokens.radii.full)
                .bg(GREEN.with_alpha(34)),
            ],
            align_items: ir_op::AlignItems::Center,
            ..Default::default()
        }
        .into()
    }
}
