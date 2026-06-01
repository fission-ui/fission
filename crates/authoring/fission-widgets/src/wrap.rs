use fission_core::ui::Node;
use fission_core::{BuildCtx, View, Widget};
use fission_ir::op::{FlexDirection, FlexWrap};
use serde::{Deserialize, Serialize};

/// A flow layout that wraps children to the next line when they exceed the
/// available width (or height, for column direction).
///
/// Uses `FlexWrap::Wrap` on the underlying `Row` or `Column` layout node.
///
/// # Fields
///
/// * `direction` - `FlexDirection::Row` (default) or `FlexDirection::Column`.
/// * `spacing` - Gap between children (applied as `gap`).
/// * `children` - The child nodes to lay out.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wrap {
    pub direction: FlexDirection,
    pub spacing: Option<f32>,
    pub children: Vec<Node>,
}

impl Default for Wrap {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            spacing: None,
            children: Vec::new(),
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for Wrap {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            match self.direction {
                FlexDirection::Row => fission_core::ui::Row {
                    children: self.children.clone(),
                    wrap: FlexWrap::Wrap,
                    gap: self.spacing,
                    ..Default::default()
                }
                .into_node(),
                FlexDirection::Column => fission_core::ui::Column {
                    children: self.children.clone(),
                    wrap: FlexWrap::Wrap,
                    gap: self.spacing,
                    ..Default::default()
                }
                .into_node(),
            }
        })
    }
}
