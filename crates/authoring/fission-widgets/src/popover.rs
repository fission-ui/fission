use fission_core::op::Color;
use fission_core::ui::{Container, GestureDetector, Node};
use fission_core::{ActionEnvelope, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Popover {
    pub id: WidgetNodeId,
    pub is_open: bool,
    pub on_toggle: Option<ActionEnvelope>,
    pub on_close: Option<ActionEnvelope>,

    pub trigger: Box<Node>,
    pub content: Box<Node>,
}

impl<S: fission_core::AppState> Widget<S> for Popover {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        // Derive stable anchor ID
        let anchor_id = NodeId::derived(self.id.as_u128(), &[0]);

        let trigger_wrapper = Container::new(*self.trigger.clone())
            .id(anchor_id)
            .flex_shrink(0.0)
            .into_node();

        // Wrap trigger in a clickable area if on_toggle provided?
        // Or assume trigger handles clicks.
        // Usually trigger handles clicks.

        if self.is_open {
            let content_node = *self.content.clone();
            let flyout_node = crate::flyout(anchor_id, content_node);
            if self.on_close.is_some() {
                let backdrop = GestureDetector {
                    on_tap: self.on_close.clone(),
                    child: Box::new(
                        Container::new(fission_core::ui::widgets::Spacer::default().into_node())
                            .bg(Color {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 0,
                            })
                            .into_node(),
                    ),
                    ..Default::default()
                }
                .into_node();

                // We need to render [Backdrop, Flyout].
                // Backdrop is ZStack layer 0. Flyout layer 1.
                use fission_core::ui::ZStack;

                let overlay = ZStack {
                    children: vec![
                        fission_core::ui::Positioned {
                            left: Some(0.0),
                            top: Some(0.0),
                            right: Some(0.0),
                            bottom: Some(0.0),
                            child: Some(Box::new(backdrop)),
                            ..Default::default()
                        }
                        .into_node(),
                        flyout_node,
                    ],
                    ..Default::default()
                }
                .into_node();

                ctx.register_portal_with_layer(fission_core::PortalLayer::Flyout, Some(self.id), overlay);
            } else {
                ctx.register_portal_with_layer(fission_core::PortalLayer::Flyout, Some(self.id), flyout_node);
            }
        }

        trigger_wrapper
    }
}
