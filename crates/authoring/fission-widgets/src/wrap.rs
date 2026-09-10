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
/// * `spacing` - Gap between adjacent items on a line.
/// * `run_spacing` - Gap between wrapped lines; defaults to `spacing`.
/// * `children` - The child nodes to lay out.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wrap {
    /// Main axis along which children are placed before wrapping.
    pub direction: FlexDirection,
    /// Optional logical-pixel gap between adjacent children on each line.
    pub spacing: Option<f32>,
    /// Optional logical-pixel gap between wrapped lines.
    pub run_spacing: Option<f32>,
    /// Children placed in declaration order.
    pub children: Vec<Widget>,
}

impl Default for Wrap {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            spacing: None,
            run_spacing: None,
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
                line_gap: this.run_spacing,
                ..Default::default()
            }
            .into(),
            FlexDirection::Column => fission_core::ui::Column {
                children: this.children.clone(),
                wrap: FlexWrap::Wrap,
                gap: this.spacing,
                line_gap: this.run_spacing,
                ..Default::default()
            }
            .into(),
        }
    }
}
