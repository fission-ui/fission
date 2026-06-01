use fission_core::ui::{Column, Node, Row};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

/// A horizontal stack that arranges children in a row with optional spacing.
///
/// Convenience wrapper around [`Row`] that exposes a simpler API. Use `into_node()`
/// to convert directly to a `Node`, or use the `Widget` implementation for
/// state-aware building.
///
/// # Example
///
/// ```rust,ignore
/// HStack {
///     spacing: Some(8.0),
///     children: vec![icon_node, label_node],
/// }.into_node()
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct HStack {
    pub children: Vec<Node>,
    pub spacing: Option<f32>,
}

impl HStack {
    pub fn into_node(self) -> Node {
        Row::<fission_core::ui::Node> {
            children: self.children,
            gap: self.spacing,
            ..Default::default()
        }
        .into()
    }
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
/// }.into_node()
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct VStack {
    pub children: Vec<Node>,
    pub spacing: Option<f32>,
}

impl VStack {
    pub fn into_node(self) -> Node {
        Column::<fission_core::ui::Node> {
            children: self.children,
            gap: self.spacing,
            ..Default::default()
        }
        .into()
    }
}

impl<S: fission_core::AppState> Widget<S> for HStack {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget(self.clone().into_node())
    }
}

impl<S: fission_core::AppState> Widget<S> for VStack {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget(self.clone().into_node())
    }
}
