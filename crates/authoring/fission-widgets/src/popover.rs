use fission_core::op::Color;
use fission_core::ui::{Container, GestureDetector, Widget};
use fission_core::{ActionEnvelope, WidgetId, WidgetIdExt};
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
    pub id: WidgetId,
    pub is_open: bool,
    pub on_toggle: Option<ActionEnvelope>,
    pub on_close: Option<ActionEnvelope>,

    pub trigger: Widget,
    pub content: Widget,
}

impl From<Popover> for Widget {
    fn from(component: Popover) -> Self {
        let (ctx, _) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        // Derive stable anchor ID
        let anchor_id = WidgetId::derived(this.id.as_u128(), &[0]);

        let trigger_wrapper = Container::new(this.trigger.clone())
            .flex_shrink(0.0)
            .id(anchor_id);

        // Wrap trigger in a clickable area if on_toggle provided?
        // Or assume trigger handles clicks.
        // Usually trigger handles clicks.

        if this.is_open {
            let content_node = this.content.clone();
            let flyout_node = crate::flyout(anchor_id, content_node);
            if this.on_close.is_some() {
                let backdrop = GestureDetector {
                    on_tap: this.on_close.clone(),
                    child: Container::new(fission_core::ui::widgets::Spacer::default())
                        .bg(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        })
                        .into(),
                    ..Default::default()
                }
                .into();

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
                            child: Some(backdrop),
                            ..Default::default()
                        }
                        .into(),
                        flyout_node,
                    ],
                    ..Default::default()
                }
                .into();

                ctx.register_portal_with_layer(
                    fission_core::PortalLayer::Flyout,
                    Some(this.id),
                    overlay,
                );
            } else {
                ctx.register_portal_with_layer(
                    fission_core::PortalLayer::Flyout,
                    Some(this.id),
                    flyout_node,
                );
            }
        }

        trigger_wrapper
    }
}
