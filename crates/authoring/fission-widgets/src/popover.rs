use fission_core::op::Color;
use fission_core::ui::{Container, GestureDetector, Node, Positioned, ZStack};
use fission_core::{ActionEnvelope, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use serde::{Deserialize, Serialize};

/// An anchor-relative popup that renders content positioned next to a trigger widget.
///
/// The trigger widget is rendered inline in the normal layout tree. When `is_open`
/// is `true`, the `content` is placed into a flyout portal positioned relative to
/// the trigger's computed rect. An optional transparent backdrop handles dismiss
/// via `on_close`.
///
/// # Fields
///
/// * `id` - Stable widget identity for the portal system.
/// * `is_open` - Controls visibility of the popup content.
/// * `on_toggle` - Action dispatched to toggle the popover.
/// * `on_close` - Action dispatched when the backdrop is tapped (if set, a backdrop is rendered).
/// * `trigger` - The inline widget that the popover is anchored to.
/// * `content` - The popup content rendered in the flyout layer.
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
    fn build(&self, ctx: &mut BuildCtx<S>, _view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            // Derive stable anchor ID
            let anchor_id = NodeId::derived(self.id.as_u128(), &[0]);

            let trigger_wrapper =
                Container::<fission_core::ui::Node>::lowered(*self.trigger.clone())
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
                            Container::<fission_core::ui::Node>::lowered(
                                fission_core::ui::widgets::Spacer::default().into_node(),
                            )
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

                    let overlay = ZStack::<fission_core::ui::Node> {
                        children: vec![
                            Positioned::<fission_core::ui::Node> {
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

                    ctx.register_portal_with_layer(
                        fission_core::PortalLayer::Flyout,
                        Some(self.id),
                        overlay,
                    );
                } else {
                    ctx.register_portal_with_layer(
                        fission_core::PortalLayer::Flyout,
                        Some(self.id),
                        flyout_node,
                    );
                }
            }

            trigger_wrapper
        })
    }
}
