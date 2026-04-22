use fission_core::ui::{Container, Node, Text};
use fission_core::{BuildCtx, View, Widget, WidgetNodeId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tooltip {
    pub id: WidgetNodeId,
    pub child: Box<Node>,
    pub text: String,
    pub is_visible: bool,
}

impl<S: fission_core::AppState> Widget<S> for Tooltip {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let theme = &view.env.theme.components.tooltip;

        let trigger_id = fission_ir::NodeId::derived(self.id.as_u128(), &[]);
        let is_hovered = view.runtime.interaction.is_hovered(trigger_id);
        let show_tooltip = self.is_visible || is_hovered;

        let trigger = Container::new(*self.child.clone())
            .id(trigger_id)
            .into_node();

        if show_tooltip {
            let tooltip_card = Container::new(
                Text::new(self.text.clone())
                    .size(theme.font_size)
                    .color(theme.text_color)
                    .max_width(220.0)
                    .into_node(),
            )
            .bg(theme.bg_color)
            .padding_all(8.0)
            .border_radius(theme.radius)
            .into_node();

            let flyout_node = crate::flyout(
                fission_ir::NodeId::derived(self.id.as_u128(), &[]),
                tooltip_card,
            );
            ctx.register_portal_with_layer(fission_core::PortalLayer::Flyout, Some(self.id), flyout_node);
        }

        trigger
    }
}
