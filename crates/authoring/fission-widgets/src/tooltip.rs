use fission_core::ui::{Container, Text, Widget};
use fission_core::{WidgetId, WidgetIdExt};
use serde::{Deserialize, Serialize};

/// A hover-activated text tooltip displayed near a trigger widget.
///
/// The tooltip appears when the trigger widget is hovered (detected via
/// `view.runtime().interaction.is_hovered`) or when `is_visible` is explicitly
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
    pub id: WidgetId,
    pub child: Widget,
    pub text: String,
    pub is_visible: bool,
}

impl From<Tooltip> for Widget {
    fn from(component: Tooltip) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let theme = &view.env().theme.components.tooltip;

        let trigger_id = fission_ir::WidgetId::derived(this.id.as_u128(), &[]);
        let is_hovered = view.runtime().interaction.is_hovered(trigger_id);
        let show_tooltip = this.is_visible || is_hovered;

        let trigger = Container::new(this.child.clone()).id(trigger_id);

        if show_tooltip {
            let style = &theme.style;
            let tooltip_card = Container::new(
                Text::new(this.text.clone())
                    .size(style.font_size.unwrap_or(theme.font_size))
                    .color(style.text_color.unwrap_or(theme.text_color))
                    .max_width(style.max_width.unwrap_or(theme.max_width)),
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
            .into();

            let flyout_node = crate::flyout(
                fission_ir::WidgetId::derived(this.id.as_u128(), &[]),
                tooltip_card,
            );
            ctx.register_portal_with_layer(
                fission_core::PortalLayer::Flyout,
                Some(this.id),
                flyout_node,
            );
        }

        trigger
    }
}
