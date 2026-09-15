//! Token groups added after the original DSP format: sizing, opacity,
//! breakpoints, layers and the default density. Every token is optional.

use super::*;

impl Package {
    /// The generated theme, moved to the design system's declared default density.
    ///
    /// Control sizes are declared once, at comfortable density. A design system
    /// whose apps should start smaller or larger sets `sizing.default_density`
    /// to `compact` or `spacious`; anything else keeps the declared sizes.
    pub(super) fn theme_finish_expr(&self, krate: &str) -> Result<String> {
        let theme = format!("{krate}::Theme {{ tokens, components, design_system }}");
        let density = self.string_token_optional("sizing.default_density", "comfortable")?;
        Ok(match density.trim().to_ascii_lowercase().as_str() {
            "compact" => format!("({theme}).with_density({krate}::Density::Compact)"),
            "spacious" => format!("({theme}).with_density({krate}::Density::Spacious)"),
            _ => theme,
        })
    }

    // The groups below are newer than the DSP format most design systems were
    // written against, so every token is optional: a design system that omits
    // one keeps compiling and gets Fission's default value.
    pub(super) fn sizing_tokens_expr(&self, krate: &str) -> Result<String> {
        let control_md = self.dsp_dimension_optional(
            "/components/button/sizes/md/height",
            self.dimension_optional("component.button.height", 36.0)?,
        )?;
        let d = |path: &str, fallback: f32| -> Result<String> {
            Ok(f32_lit(self.dimension_optional(path, fallback)?))
        };
        Ok(format!(
            "{krate}::SizingTokens {{ min_pointer_target: {}, min_touch_target: {}, control_sm: {}, control_md: {}, control_lg: {}, control_xl: {}, icon_xs: {}, icon_sm: {}, icon_md: {}, icon_lg: {}, icon_xl: {}, border_hairline: {}, border_thick: {}, focus_ring_width: {}, focus_ring_offset: {}, density_step: {} }}",
            d("sizing.target.pointer", 24.0)?,
            d("sizing.target.touch", 48.0)?,
            d("sizing.control.sm", 32.0)?,
            f32_lit(self.dimension_optional("sizing.control.md", control_md)?),
            d("sizing.control.lg", 40.0)?,
            d("sizing.control.xl", 48.0)?,
            d("sizing.icon.xs", 12.0)?,
            d("sizing.icon.sm", 16.0)?,
            d("sizing.icon.md", 20.0)?,
            d("sizing.icon.lg", 24.0)?,
            d("sizing.icon.xl", 32.0)?,
            d("sizing.border.hairline", 1.0)?,
            d("sizing.border.thick", 2.0)?,
            d("sizing.focus_ring.width", 3.0)?,
            d("sizing.focus_ring.offset", 0.0)?,
            d("sizing.density_step", 4.0)?,
        ))
    }

    pub(super) fn opacity_tokens_expr(&self, krate: &str) -> Result<String> {
        let n = |path: &str, fallback: f32| -> Result<String> {
            Ok(f32_lit(
                self.number_optional(path, fallback)?.clamp(0.0, 1.0),
            ))
        };
        Ok(format!(
            "{krate}::OpacityTokens {{ disabled: {}, muted: {}, scrim: {}, hover_layer: {}, pressed_layer: {}, focus_layer: {}, selected_layer: {}, dragged_layer: {} }}",
            n("opacity.disabled", 0.5)?,
            n("opacity.muted", 0.72)?,
            n("opacity.scrim", 0.4)?,
            n("opacity.state.hover", 0.08)?,
            n("opacity.state.pressed", 0.12)?,
            n("opacity.state.focus", 0.12)?,
            n("opacity.state.selected", 0.12)?,
            n("opacity.state.dragged", 0.16)?,
        ))
    }

    pub(super) fn breakpoint_tokens_expr(&self, krate: &str) -> Result<String> {
        let d = |path: &str, fallback: f32| -> Result<String> {
            Ok(f32_lit(self.dimension_optional(path, fallback)?))
        };
        Ok(format!(
            "{krate}::BreakpointTokens {{ compact_max: {}, medium_max: {}, expanded_max: {}, large_max: {} }}",
            d("breakpoint.compact_max", 600.0)?,
            d("breakpoint.medium_max", 840.0)?,
            d("breakpoint.expanded_max", 1200.0)?,
            d("breakpoint.large_max", 1600.0)?,
        ))
    }

    pub(super) fn layer_tokens_expr(&self, krate: &str) -> Result<String> {
        let n = |path: &str, fallback: f32| -> Result<String> {
            Ok(format!(
                "{}i32",
                self.number_optional(path, fallback)?.round() as i32
            ))
        };
        Ok(format!(
            "{krate}::LayerTokens {{ base: {}, raised: {}, sticky: {}, dropdown: {}, overlay: {}, modal: {}, popover: {}, toast: {}, tooltip: {} }}",
            n("layer.base", 0.0)?,
            n("layer.raised", 10.0)?,
            n("layer.sticky", 100.0)?,
            n("layer.dropdown", 1000.0)?,
            n("layer.overlay", 1100.0)?,
            n("layer.modal", 1200.0)?,
            n("layer.popover", 1300.0)?,
            n("layer.toast", 1400.0)?,
            n("layer.tooltip", 1500.0)?,
        ))
    }

    pub(super) fn number_optional(&self, path: &str, fallback: f32) -> Result<f32> {
        if !self.tokens.contains(path) {
            return Ok(fallback);
        }
        let value = self.resolve_token_string(path)?;
        value
            .parse::<f32>()
            .or_else(|_| parse_dimension(&value))
            .with_context(|| format!("invalid number token {path} = {value}"))
    }

    pub(super) fn string_token_optional(&self, path: &str, fallback: &str) -> Result<String> {
        if !self.tokens.contains(path) {
            return Ok(fallback.to_string());
        }
        self.resolve_token_string(path)
    }
}
