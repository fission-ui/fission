// Or make a custom Flex?
// Row is Flex Row.
use fission_core::ui::{Container, Widget};
use fission_ir::op::FlexWrap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleGrid {
    pub min_child_width: f32,
    pub gap: Option<f32>,
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
