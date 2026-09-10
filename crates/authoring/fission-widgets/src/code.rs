use fission_core::op::{Color, Fill};
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
        Container::new(
            Text::new(this.text.clone())
                .size(12.0)
                .color(tokens.colors.text_primary),
        )
        .bg(Color {
            r: 245,
            g: 245,
            b: 245,
            a: 255,
        })
        .border(
            Color {
                r: 200,
                g: 200,
                b: 200,
                a: 255,
            },
            1.0,
        )
        .border_radius(4.0)
        .padding_all(4.0)
        .into()
    }
}
