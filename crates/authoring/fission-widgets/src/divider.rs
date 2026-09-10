use fission_core::ui::{Container, Widget};
use fission_ir::op::Color;
use serde::{Deserialize, Serialize};

/// The direction of a [`Divider`] line.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Orientation {
    /// A line that expands across the available width.
    Horizontal,
    /// A line that expands across the available height.
    Vertical,
}

impl Default for Orientation {
    fn default() -> Self {
        Orientation::Horizontal
    }
}

/// A visual separator line.
///
/// Renders a thin line in the theme's `border` color. Defaults to horizontal
/// orientation. The divider uses `flex_grow: 1.0` to fill the available width
/// (horizontal) or height (vertical).
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Divider {
    /// Axis along which the separator is drawn.
    pub orientation: Orientation,
    /// Line thickness in logical pixels. Defaults to one pixel.
    pub thickness: Option<f32>,
    /// Line colour. Defaults to the active theme's border colour.
    pub color: Option<Color>,
    /// Optional alternating painted and unpainted lengths.
    pub dash_pattern: Option<Vec<f32>>,
}

impl From<Divider> for Widget {
    fn from(component: Divider) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let thickness = this.thickness.unwrap_or(1.0).max(0.0);
        let color = this.color.unwrap_or(tokens.colors.border);
        let (w, h) = match this.orientation {
            Orientation::Horizontal => (f32::NAN, thickness), // Auto width
            Orientation::Vertical => (thickness, f32::NAN),   // Auto height
        };

        let mut c = Container::new(fission_core::ui::Row::default()); // Empty
        if let Some(pattern) = &this.dash_pattern {
            c = c
                .border(color, thickness.max(f32::EPSILON))
                .border_dash(pattern.clone());
        } else {
            c = c.bg(color);
        }

        if w.is_nan() {
            // Container width default is Auto (None)
        } else {
            c = c.width(w);
        }

        if h.is_nan() {
            // Container height default is Auto (None)
        } else {
            c = c.height(h);
        }

        c = c.flex_grow(1.0);

        c.into()
    }
}
