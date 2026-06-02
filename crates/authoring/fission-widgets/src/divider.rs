use fission_core::ui::{Container, Widget};
use serde::{Deserialize, Serialize};

/// The direction of a [`Divider`] line.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Default for Orientation {
    fn default() -> Self {
        Orientation::Horizontal
    }
}

/// A 1px visual separator line.
///
/// Renders a thin line in the theme's `border` color. Defaults to horizontal
/// orientation. The divider uses `flex_grow: 1.0` to fill the available width
/// (horizontal) or height (vertical).
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Divider {
    pub orientation: Orientation,
}

impl From<Divider> for Widget {
    fn from(component: Divider) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let (w, h) = match this.orientation {
            Orientation::Horizontal => (f32::NAN, 1.0), // Auto width
            Orientation::Vertical => (1.0, f32::NAN),   // Auto height
        };

        let mut c = Container::new(fission_core::ui::Row::default()) // Empty
            .bg(tokens.colors.border);

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
