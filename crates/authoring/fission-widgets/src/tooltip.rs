use fission_core::ui::{Container, Node, Text, TextContent, Positioned};
use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId};
use fission_core::op::{Color, BoxShadow};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tooltip {
    pub id: WidgetNodeId,
    pub child: Box<Node>,
    pub text: String,
}

impl<S: fission_core::AppState> Widget<S> for Tooltip {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let node_id = NodeId::derived(self.id.as_u128(), &[]);
        
        let trigger = Container::new(*self.child.clone())
            .id(node_id)
            .into_node();

        // Check hover state from RuntimeState
        // View has access to interaction state?
        // `view.interaction.is_hovered(node_id)`?
        // `View` struct needs to expose `interaction`.
        // `View` has `runtime_state`.
        
        let is_hovered = view.runtime.interaction.is_hovered(node_id);

        if is_hovered {
            if let Some(rect) = view.get_rect(self.id) {
                let x = rect.origin.x;
                let y = rect.bottom() + 4.0;

                let content = Positioned {
                    left: Some(x),
                    top: Some(y),
                    child: Some(Box::new(
                        Container::new(
                            Text { 
                                content: TextContent::Literal(self.text.clone()), 
                                color: Some(Color::WHITE),
                                font_size: Some(12.0),
                                ..Default::default() 
                            }.into()
                        )
                        .bg(Color { r: 50, g: 50, b: 50, a: 255 })
                        .border_radius(4.0)
                        .padding_all(4.0)
                        .into_node()
                    )),
                    ..Default::default()
                }.build(ctx, view);

                ctx.register_portal(content);
            }
        }

        trigger
    }
}
