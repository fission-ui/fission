use crate::stack::{HStack, VStack};
use crate::{flyout, Divider, Icon};
use fission_core::op::{BoxShadow, Color};
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Scroll, Text, TextContent,
};
use fission_core::{ActionEnvelope, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use fission_icons::material;
use serde::{Deserialize, Serialize};

/// A single entry in a [`Menu`]: label text, optional SVG icon, and selection action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub on_select: Option<ActionEnvelope>,
}

/// A vertical dropdown menu rendered as a scrollable list of [`MenuItem`] entries.
///
/// The menu is displayed inside a bordered, elevated container with rounded corners.
/// Items are separated by [`Divider`](crate::Divider) lines. When the total item
/// height exceeds `max_height` (default 300px), a scrollbar appears.
///
/// `Menu` is typically not used directly -- it is composed by [`MenuButton`],
/// [`Select`], and other selection widgets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub width: Option<f32>,
    pub max_height: Option<f32>,
}

impl<S: fission_core::AppState> Widget<S> for Menu {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            let tokens = &view.env.theme.tokens;
            let mut menu_items = Vec::new();

            let item_width = self.width.unwrap_or(200.0);

            for (idx, item) in self.items.iter().enumerate() {
                let mut row_children = Vec::new();
                if let Some(icon_path) = &item.icon {
                    row_children.push(Icon::svg(icon_path.clone()).size(18.0).into_node());
                }
                row_children.push(
                    Text::new(item.label.clone())
                        .size(14.0)
                        .flex_grow(1.0)
                        .into_node(),
                );

                menu_items.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        content_align: ButtonContentAlign::Start,
                        child: Some(Box::new(
                            Container::new(
                                HStack {
                                    spacing: Some(12.0),
                                    children: row_children,
                                }
                                .into_node(),
                            )
                            .flex_grow(1.0)
                            .into_node(),
                        )),
                        on_press: item.on_select.clone(),
                        width: Some(item_width),
                        height: Some(36.0),
                        padding: Some([12.0, 12.0, 0.0, 0.0]),
                        ..Default::default()
                    }
                    .into(),
                );

                if idx + 1 < self.items.len() {
                    menu_items.push(
                        Divider {
                            orientation: crate::divider::Orientation::Horizontal,
                        }
                        .build_node(ctx, view),
                    );
                }
            }

            let content = VStack {
                spacing: Some(2.0),
                children: menu_items,
            }
            .into_node();

            let estimated_item_height = 36.0;
            let estimated_dividers = self.items.len().saturating_sub(1) as f32;
            let estimated_height =
                (self.items.len() as f32 * estimated_item_height) + estimated_dividers + 8.0;
            let max_h = self.max_height.unwrap_or(300.0);
            let popup_height = estimated_height.min(max_h);
            let scroll_height = Some(popup_height);
            let show_scrollbar = estimated_height > max_h + 0.5;

            let scrollable_content = Scroll {
                child: Some(Box::new(content)),
                height: scroll_height,
                width: self.width,
                show_scrollbar,
                ..Default::default()
            }
            .into_node();

            Container::new(scrollable_content)
                // Keep the menu surface bounded to the scroll viewport.
                .height(popup_height + 8.0)
                .bg(tokens.colors.surface)
                .border(tokens.colors.border, 1.0)
                .border_radius(tokens.radii.medium)
                .shadow(tokens.elevations.level2.unwrap_or(BoxShadow {
                    color: Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 40,
                    },
                    blur_radius: 8.0,
                    offset: (0.0, 4.0),
                }))
                .padding_all(4.0)
                .into_node()
        })
    }
}

/// A button that toggles a [`Menu`] popover when clicked.
///
/// Renders an outline button with a label and a chevron icon. When `is_open`
/// is `true`, a flyout portal containing the menu items is displayed anchored
/// to the button.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuButton {
    pub id: WidgetNodeId,
    pub label: String,
    pub items: Vec<MenuItem>,
    pub is_open: bool,
    pub on_toggle: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for MenuButton {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            let tokens = &view.env.theme.tokens;
            let anchor_id = NodeId::derived(self.id.as_u128(), &[]);

            // Trigger Button
            let trigger = Button {
                id: Some(anchor_id),
                variant: ButtonVariant::Outline,
                content_align: ButtonContentAlign::Start,
                child: Some(Box::new(
                    HStack {
                        spacing: Some(6.0),
                        children: vec![
                            Text {
                                content: TextContent::Literal(self.label.clone()),
                                color: Some(tokens.colors.primary),
                                ..Default::default()
                            }
                            .into_node(),
                            Icon::svg(material::navigation::expand_more::regular())
                                .size(16.0)
                                .color(tokens.colors.text_secondary)
                                .into_node(),
                        ],
                    }
                    .into_node(),
                )),
                on_press: self.on_toggle.clone(),
                height: Some(40.0),
                padding: Some([12.0, 12.0, 0.0, 0.0]),
                ..Default::default()
            }
            .into();

            // Menu Overlay
            if self.is_open {
                let menu_content = Menu {
                    items: self.items.clone(),
                    width: Some(200.0),
                    max_height: Some(300.0),
                }
                .build_node(ctx, view);

                let flyout_node = flyout(anchor_id, menu_content);
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
