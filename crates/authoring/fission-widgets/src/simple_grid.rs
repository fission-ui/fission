// Or make a custom Flex?
// Row is Flex Row.
use fission_core::ui::{Container, Node};
use fission_core::{BuildCtx, View, Widget};
use fission_ir::op::FlexWrap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleGrid {
    pub min_child_width: f32,
    pub gap: Option<f32>,
    pub children: Vec<Node>,
}

impl<S: fission_core::AppState> Widget<S> for SimpleGrid {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            let wrapped_children: Vec<Node> = self
                .children
                .iter()
                .map(|child| {
                    Container::new(child.clone())
                        .flex_grow(1.0)
                        .min_width(self.min_child_width)
                        .into_node()
                })
                .collect();

            // Use Row with Wrap
            fission_core::ui::Row {
                children: wrapped_children,
                wrap: FlexWrap::Wrap,
                gap: self.gap,
                ..Default::default()
            }
            .into_node()
        })
    }
}
