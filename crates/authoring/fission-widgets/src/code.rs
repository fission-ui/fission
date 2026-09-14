use fission_core::op::Fill;
use fission_core::ui::{Container, Text, Widget};
use serde::{Deserialize, Serialize};

/// Inline presentation for source code or another monospace fragment.
///
/// This widget displays text only; it does not perform syntax highlighting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Code {
    /// Literal code text to display.
    pub text: String,
}

impl From<Code> for Widget {
    fn from(component: Code) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let style = &view.env().theme.components.code.style;
        let tokens = &view.env().theme.tokens;
        let mut text = Text::new(this.text.clone())
            .size(style.font_size.unwrap_or(tokens.typography.font_size_xs))
            .family(
                style
                    .font_family
                    .clone()
                    .unwrap_or_else(|| tokens.typography.font_family_mono.clone()),
            )
            .color(style.text_color.unwrap_or(tokens.colors.text_primary));
        if let Some(weight) = style.font_weight {
            text = text.weight(weight);
        }
        if let Some(line_height) = style.line_height {
            text = text.line_height(line_height);
        }
        if let Some(letter_spacing) = style.letter_spacing {
            text = text.letter_spacing(letter_spacing);
        }

        let mut surface = Container::new(text)
            .padding(style.padding_box(tokens.spacing.xs, tokens.spacing.xs / 2.0))
            .border_radius(style.radius.unwrap_or(tokens.radii.small))
            .shadows(style.outer_shadows());
        if let Some(background) = style.background.clone() {
            surface = surface.bg_fill(background);
        }
        if let Some(border) = style.border.as_ref() {
            if let Fill::Solid(color) = &border.fill {
                surface = surface.border(*color, border.width);
            }
        }
        surface.into()
    }
}

/// Keyboard-key label with a compact keycap border and background.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Kbd {
    /// Key name or shortcut displayed on the keycap.
    pub text: String,
}

impl From<Kbd> for Widget {
    fn from(component: Kbd) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        // A keycap is the code recipe with a raised surface, not a separate
        // palette. The previous fixed light grey rendered a near-white cap in a
        // dark theme, with the border invisible against it.
        let style = &view.env().theme.components.code.style;
        let mut cap = Container::new(
            Text::new(this.text.clone())
                .size(style.font_size.unwrap_or(tokens.typography.font_size_xs))
                .family(
                    style
                        .font_family
                        .clone()
                        .unwrap_or_else(|| tokens.typography.font_family_mono.clone()),
                )
                .color(style.text_color.unwrap_or(tokens.colors.text_primary)),
        )
        .bg_fill(
            style
                .background
                .clone()
                .unwrap_or(Fill::Solid(tokens.colors.surface_raised)),
        )
        .border_radius(style.radius.unwrap_or(tokens.radii.small))
        .padding(style.padding_box(tokens.spacing.xs, tokens.spacing.xs / 2.0));
        let border_color = style
            .border
            .as_ref()
            .and_then(|border| match &border.fill {
                Fill::Solid(color) => Some(*color),
                _ => None,
            })
            .unwrap_or(tokens.colors.border);
        cap = cap.border(border_color, style.border.as_ref().map_or(1.0, |b| b.width));
        cap.into()
    }
}
