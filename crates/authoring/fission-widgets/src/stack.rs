use fission_core::ui::{Column, Row, Widget};
use serde::{Deserialize, Serialize};

/// A horizontal stack that arranges children in a row with optional spacing.
///
/// Convenience wrapper around [`Row`] that exposes a simpler API and converts
/// into the closed [`Widget`] tree value.
///
/// # Example
///
/// ```rust,ignore
/// HStack {
///     spacing: Some(8.0),
///     children: vec![icon_node, label_node],
/// }.into()
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct HStack {
    pub children: Vec<Widget>,
    pub spacing: Option<f32>,
}

/// A vertical stack that arranges children in a column with optional spacing.
///
/// Convenience wrapper around [`Column`] that exposes a simpler API.
///
/// # Example
///
/// ```rust,ignore
/// VStack {
///     spacing: Some(12.0),
///     children: vec![title_node, body_node, footer_node],
/// }.into()
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct VStack {
    pub children: Vec<Widget>,
    pub spacing: Option<f32>,
}

impl From<HStack> for Widget {
    fn from(component: HStack) -> Self {
        Row {
            children: component.children,
            gap: component.spacing,
            ..Default::default()
        }
        .into()
    }
}

impl From<VStack> for Widget {
    fn from(component: VStack) -> Self {
        Column {
            children: component.children,
            gap: component.spacing,
            ..Default::default()
        }
        .into()
    }
}
