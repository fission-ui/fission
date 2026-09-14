use super::state::DocsState;
use fission::op::{AlignItems, JustifyContent};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct PlatformAtlas;

impl From<PlatformAtlas> for Widget {
    fn from(_atlas: PlatformAtlas) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Row {
                    children: vec![
                        AtlasTarget::new("Android", "A").into(),
                        AtlasTarget::new("Web", "◎").into(),
                        AtlasTarget::new("iOS", "●").into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    justify_content: JustifyContent::SpaceBetween,
                    ..Default::default()
                }
                .into(),
                Container::new(Image::asset("/img/fission-mark.svg").size(92.0, 108.0))
                    .padding_lengths(Length::all(Length::points(tokens.spacing.xl)))
                    .bg(tokens.colors.primary)
                    .border_radius(tokens.radii.xl)
                    .into(),
                Row {
                    children: vec![
                        AtlasTarget::new("Linux", "L").into(),
                        AtlasTarget::new("macOS", "M").into(),
                        AtlasTarget::new("Windows", "⊞").into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    justify_content: JustifyContent::SpaceBetween,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.l),
            align_items: AlignItems::Center,
            semantics: Some(super::home_widgets::site_semantics("site-platform-atlas")),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.xl)))
        .bg(tokens.colors.primary_subtle)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xl)
        .min_height_length(Length::points(410.0))
        .into()
    }
}

#[derive(Clone, Debug)]
struct AtlasTarget {
    label: &'static str,
    glyph: &'static str,
}

impl AtlasTarget {
    fn new(label: &'static str, glyph: &'static str) -> Self {
        Self { label, glyph }
    }
}

impl From<AtlasTarget> for Widget {
    fn from(target: AtlasTarget) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new(target.glyph)
                    .size(20.0)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(target.label)
                    .size(11.0)
                    .color(tokens.colors.text_muted)
                    .into(),
            ],
            gap: Some(tokens.spacing.xs),
            align_items: AlignItems::Center,
            semantics: Some(super::home_widgets::site_semantics(format!(
                "site-atlas-target:{}",
                target.label.to_lowercase()
            ))),
            ..Default::default()
        })
        .width_length(Length::points(104.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
        .bg(tokens.colors.surface)
        .border_radius(tokens.radii.large)
        .into()
    }
}
