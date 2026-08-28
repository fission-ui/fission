// Or make a custom Flex?
// Row is Flex Row.
use fission_core::ui::{Container, Widget};
use fission_ir::op::FlexWrap;
use serde::{Deserialize, Serialize};

/// Responsive wrapping grid whose children share a minimum width.
///
/// Children expand evenly across each row and wrap when another
/// `min_child_width` item no longer fits. Use [`fission_core::ui::Grid`] when
/// the application needs explicit rows, columns, or named placements.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleGrid {
    /// Smallest logical width assigned to an item before wrapping occurs.
    pub min_child_width: f32,
    /// Optional horizontal and vertical gap between items.
    pub gap: Option<f32>,
    /// Items laid out in declaration order.
    pub children: Vec<Widget>,
}

impl From<SimpleGrid> for Widget {
    fn from(component: SimpleGrid) -> Self {
        let this = &component;

        let wrapped_children: Vec<Widget> = this
            .children
            .iter()
            .map(|child| {
                Container::new(child.clone())
                    .flex_grow(1.0)
                    .min_width(this.min_child_width)
                    .into()
            })
            .collect();

        // Use Row with Wrap
        fission_core::ui::Row {
            children: wrapped_children,
            wrap: FlexWrap::Wrap,
            gap: this.gap,
            ..Default::default()
        }
        .into()
    }
}
