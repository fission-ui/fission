use fission_core::ui::Widget;
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
    pub children: Vec<Widget>,
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

impl From<Wrap> for Widget {
    fn from(component: Wrap) -> Self {
        let this = &component;

        match this.direction {
            FlexDirection::Row => fission_core::ui::Row {
                children: this.children.clone(),
                wrap: FlexWrap::Wrap,
                gap: this.spacing,
                ..Default::default()
            }
            .into(),
            FlexDirection::Column => fission_core::ui::Column {
                children: this.children.clone(),
                wrap: FlexWrap::Wrap,
                gap: this.spacing,
                ..Default::default()
            }
            .into(),
        }
    }
}
