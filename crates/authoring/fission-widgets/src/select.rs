use crate::stack::{HStack, VStack};
use crate::{flyout, Icon, Menu, MenuItem};
use fission_core::op::{BoxShadow, Color};
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Node, Positioned, Row, Text, TextContent,
};
use fission_core::{ActionEnvelope, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use fission_icons::material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectItem {
    pub label: String,
    pub icon: Option<String>,
    pub on_select: ActionEnvelope,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Select {
    pub id: WidgetNodeId,
    pub selected_label: Option<String>,
    pub items: Vec<SelectItem>,
    pub is_open: bool,
    pub on_toggle: Option<ActionEnvelope>,
    pub placeholder: String,
    pub width: Option<f32>,
}

impl Default for Select {
    fn default() -> Self {
        Self {
            id: WidgetNodeId::explicit("select"),
            selected_label: None,
            items: Vec::new(),
            is_open: false,
            on_toggle: None,
            placeholder: "Select...".into(),
            width: Some(200.0),
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for Select {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let anchor_id = NodeId::derived(self.id.as_u128(), &[]);

        let display_label = self.selected_label.as_deref().unwrap_or(&self.placeholder);
        let label_color = if self.selected_label.is_some() {
            tokens.colors.text_primary
        } else {
            tokens.colors.text_secondary
        };

        // Trigger Button content
        let trigger_content = HStack {
            spacing: Some(8.0),
            children: vec![
                Text::new(display_label.to_string())
                    .color(label_color)
                    .into_node(),
                // Spacer to push chevron to the right
                fission_core::ui::widgets::spacer::Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
                Icon::svg(material::navigation::expand_more::regular())
                    .size(20.0)
                    .color(tokens.colors.text_secondary)
                    .into_node(),
            ],
        }
        .into_node();

        let trigger = Button {
            id: Some(anchor_id),
            variant: ButtonVariant::Outline,
            content_align: ButtonContentAlign::Start,
            child: Some(Box::new(trigger_content)),
            on_press: self.on_toggle.clone(),
            width: self.width,
            ..Default::default()
        }
        .into();

        if self.is_open {
            let menu_items = self
                .items
                .iter()
                .map(|item| MenuItem {
                    label: item.label.clone(),
                    icon: item.icon.clone(),
                    on_select: Some(item.on_select.clone()),
                })
                .collect();

            let menu = Menu {
                items: menu_items,
                width: self.width,
                max_height: Some(300.0),
            }
            .build(ctx, view);

            let flyout_node = flyout(anchor_id, menu);
            ctx.register_portal_with_layer(fission_core::PortalLayer::Flyout, Some(self.id), flyout_node);
        }

        trigger
    }
}
