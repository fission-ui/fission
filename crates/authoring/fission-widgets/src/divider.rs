use fission_core::ui::{Container, SemanticsRegion, Widget};
use fission_ir::op::Color;
use fission_ir::{Role, SemanticOrientation};
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
/// orientation. It fills its long axis without participating in main-axis flex
/// growth, so its configured thickness stays fixed inside a stack.
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

        let recipe = view.env().theme.recipe("divider");
        let thickness = this
            .thickness
            .or(recipe.base.height)
            .unwrap_or(1.0)
            .max(0.0);
        let color = this
            .color
            .or_else(|| match recipe.base.background {
                Some(fission_core::op::Fill::Solid(color)) => Some(color),
                _ => None,
            })
            .unwrap_or(tokens.colors.divider);
        let mut c = Container::new(fission_core::ui::Row::default()); // Empty
        if let Some(pattern) = &this.dash_pattern {
            c = c
                .border(color, thickness.max(f32::EPSILON))
                .border_dash(pattern.clone());
        } else {
            c = c.bg(color);
        }

        c = match this.orientation {
            Orientation::Horizontal => c.height(thickness),
            Orientation::Vertical => c.width(thickness),
        };
        c = c.flex_grow(0.0).flex_shrink(0.0);

        // A separator carries structure, not just paint: it tells a reader that
        // the groups either side of it are distinct.
        SemanticsRegion::new(c)
            .role(Role::Separator)
            .orientation(match this.orientation {
                Orientation::Horizontal => SemanticOrientation::Horizontal,
                Orientation::Vertical => SemanticOrientation::Vertical,
            })
            .into()
    }
}
