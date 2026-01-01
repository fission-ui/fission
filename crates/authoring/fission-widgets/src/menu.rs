use fission_core::ui::{Button, ButtonVariant, ButtonContentAlign, Container, Node, Text, TextContent, Positioned, Scroll, Row};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope, WidgetNodeId, NodeId};
use fission_core::op::{Color, BoxShadow};
use crate::stack::{VStack, HStack};
use crate::{flyout, Icon};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub on_select: Option<ActionEnvelope>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub width: Option<f32>,
    pub max_height: Option<f32>,
}

impl<S: fission_core::AppState> Widget<S> for Menu {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let mut menu_items = Vec::new();

        for item in &self.items {
            let mut row_children = Vec::new();
            if let Some(icon_path) = &item.icon {
                row_children.push(Icon::svg(icon_path.clone()).size(18.0).into_node());
            }
            row_children.push(Text::new(item.label.clone()).into_node());

            menu_items.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(Box::new(
                        HStack {
                            spacing: Some(12.0),
                            children: row_children,
                        }.into_node()
                    )),
                    on_press: item.on_select.clone(),
                    width: Some(self.width.unwrap_or(200.0) - 8.0),
                    ..Default::default()
                }.into()
            );
        }

        let content = VStack {
            spacing: Some(2.0),
            children: menu_items,
        }.into_node();

        let scrollable_content = Scroll {
            child: Some(Box::new(content)),
            height: self.max_height,
            show_scrollbar: true,
            ..Default::default()
        }.into_node();

        Container::new(scrollable_content)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.medium)
            .shadow(tokens.elevations.level2.unwrap_or(BoxShadow {
                color: Color { r: 0, g: 0, b: 0, a: 40 },
                blur_radius: 8.0,
                offset: (0.0, 4.0),
            }))
            .padding_all(4.0)
            .into_node()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuButton {
    pub id: WidgetNodeId,
    pub label: String,
    pub items: Vec<MenuItem>,
    pub is_open: bool,
    pub on_toggle: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for MenuButton {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let anchor_id = NodeId::derived(self.id.as_u128(), &[]);

        // Trigger Button
        let trigger = Button {
            id: Some(anchor_id),
            variant: ButtonVariant::Outline,
            content_align: ButtonContentAlign::Start,
            child: Some(Box::new(
                Text { 
                    content: TextContent::Literal(self.label.clone()), 
                    color: Some(tokens.colors.primary),
                    ..Default::default() 
                }.into()
            )),
            on_press: self.on_toggle.clone(),
            ..Default::default()
        }.into();

        // Menu Overlay
        if self.is_open {
            let menu_content = Menu {
                items: self.items.clone(),
                width: Some(200.0),
                max_height: Some(300.0),
            }.build(ctx, view);

            let flyout_node = flyout(anchor_id, menu_content);
            ctx.register_portal_with_layer(fission_core::PortalLayer::Flyout, flyout_node);
        }

        trigger
    }
}
