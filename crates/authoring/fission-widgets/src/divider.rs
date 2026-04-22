use fission_core::ui::{Container, Node};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

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

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Divider {
    pub orientation: Orientation,
}

impl<S: fission_core::AppState> Widget<S> for Divider {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;

        let (w, h) = match self.orientation {
            Orientation::Horizontal => (f32::NAN, 1.0), // Auto width
            Orientation::Vertical => (1.0, f32::NAN),   // Auto height
        };

        let mut c = Container::new(fission_core::ui::Row::default().into()) // Empty
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

        c.into_node()
    }
}
