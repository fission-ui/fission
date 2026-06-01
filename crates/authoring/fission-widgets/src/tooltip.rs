use fission_core::ui::{Container, Node, Text};
use fission_core::{BuildCtx, View, Widget, WidgetNodeId};
use serde::{Deserialize, Serialize};

/// A hover-activated text tooltip displayed near a trigger widget.
///
/// The tooltip appears when the trigger widget is hovered (detected via
/// `view.runtime.interaction.is_hovered`) or when `is_visible` is explicitly
/// set to `true`. The tooltip card is styled using `TooltipTheme` and rendered
/// in the flyout portal layer.
///
/// # Fields
///
/// * `id` - Stable widget identity.
/// * `child` - The trigger widget that the tooltip is attached to.
/// * `text` - The tooltip message text (max width 220px).
/// * `is_visible` - Force the tooltip visible regardless of hover state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tooltip {
    pub id: WidgetNodeId,
    pub child: Box<Node>,
    pub text: String,
    pub is_visible: bool,
}

impl<S: fission_core::AppState> Widget<S> for Tooltip {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            let theme = &view.env.theme.components.tooltip;

            let trigger_id = fission_ir::NodeId::derived(self.id.as_u128(), &[]);
            let is_hovered = view.runtime.interaction.is_hovered(trigger_id);
            let show_tooltip = self.is_visible || is_hovered;

            let trigger = Container::<fission_core::ui::Node>::lowered(*self.child.clone())
                .id(trigger_id)
                .into_node();

            if show_tooltip {
                let style = &theme.style;
                let tooltip_card = Container::<fission_core::ui::Node>::lowered(
                    Text::new(self.text.clone())
                        .size(style.font_size.unwrap_or(theme.font_size))
                        .color(style.text_color.unwrap_or(theme.text_color))
                        .max_width(style.max_width.unwrap_or(theme.max_width))
                        .into_node(),
                )
                .bg_fill(
                    style
                        .background
                        .clone()
                        .unwrap_or(fission_core::op::Fill::Solid(theme.bg_color)),
                )
                .padding(style.padding_box(theme.padding_x, theme.padding_y))
                .border_radius(style.radius.unwrap_or(theme.radius))
                .shadows(style.outer_shadows())
                .into_node();

                let flyout_node = crate::flyout(
                    fission_ir::NodeId::derived(self.id.as_u128(), &[]),
                    tooltip_card,
                );
                ctx.register_portal_with_layer(
                    fission_core::PortalLayer::Flyout,
                    Some(self.id),
                    flyout_node,
                );
            }

            trigger
        })
    }
}
