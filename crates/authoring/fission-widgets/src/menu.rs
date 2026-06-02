use crate::stack::{HStack, VStack};
use crate::{flyout, Divider, Icon};
use fission_core::op::{BoxShadow, Color};
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Scroll, Text, TextContent, Widget,
};
use fission_core::{ActionEnvelope, WidgetId};
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

impl From<Menu> for Widget {
    fn from(component: Menu) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let mut menu_items = Vec::new();

        let item_width = this.width.unwrap_or(200.0);

        for (idx, item) in this.items.iter().enumerate() {
            let mut row_children = Vec::new();
            if let Some(icon_path) = &item.icon {
                row_children.push(Icon::svg(icon_path.clone()).size(18.0).into());
            }
            row_children.push(
                Text::new(item.label.clone())
                    .size(14.0)
                    .flex_grow(1.0)
                    .into(),
            );

            menu_items.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(
                        Container::new(HStack {
                            spacing: Some(12.0),
                            children: row_children,
                        })
                        .flex_grow(1.0)
                        .into(),
                    ),
                    on_press: item.on_select.clone(),
                    width: Some(item_width),
                    height: Some(36.0),
                    padding: Some([12.0, 12.0, 0.0, 0.0]),
                    ..Default::default()
                }
                .into(),
            );

            if idx + 1 < this.items.len() {
                menu_items.push(
                    Divider {
                        orientation: crate::divider::Orientation::Horizontal,
                    }
                    .into(),
                );
            }
        }

        let content = VStack {
            spacing: Some(2.0),
            children: menu_items,
        }
        .into();

        let estimated_item_height = 36.0;
        let estimated_dividers = this.items.len().saturating_sub(1) as f32;
        let estimated_height =
            (this.items.len() as f32 * estimated_item_height) + estimated_dividers + 8.0;
        let max_h = this.max_height.unwrap_or(300.0);
        let popup_height = estimated_height.min(max_h);
        let scroll_height = Some(popup_height);
        let show_scrollbar = estimated_height > max_h + 0.5;

        let scrollable_content: Widget = Scroll {
            child: Some(content),
            height: scroll_height,
            width: this.width,
            show_scrollbar,
            ..Default::default()
        }
        .into();

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
            .into()
    }
}

/// A button that toggles a [`Menu`] popover when clicked.
///
/// Renders an outline button with a label and a chevron icon. When `is_open`
/// is `true`, a flyout portal containing the menu items is displayed anchored
/// to the button.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuButton {
    pub id: WidgetId,
    pub label: String,
    pub items: Vec<MenuItem>,
    pub is_open: bool,
    pub on_toggle: Option<ActionEnvelope>,
}

impl From<MenuButton> for Widget {
    fn from(component: MenuButton) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let anchor_id = WidgetId::derived(this.id.as_u128(), &[]);

        // Trigger Button
        let trigger = Button {
            id: Some(anchor_id.into()),
            variant: ButtonVariant::Outline,
            content_align: ButtonContentAlign::Start,
            child: Some(
                HStack {
                    spacing: Some(6.0),
                    children: vec![
                        Text {
                            content: TextContent::Literal(this.label.clone()),
                            color: Some(tokens.colors.primary),
                            ..Default::default()
                        }
                        .into(),
                        Icon::svg(material::navigation::expand_more::regular())
                            .size(16.0)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                }
                .into(),
            ),
            on_press: this.on_toggle.clone(),
            height: Some(40.0),
            padding: Some([12.0, 12.0, 0.0, 0.0]),
            ..Default::default()
        }
        .into();

        // Menu Overlay
        if this.is_open {
            let menu_content = Menu {
                items: this.items.clone(),
                width: Some(200.0),
                max_height: Some(300.0),
            }
            .into();

            let flyout_node = flyout(anchor_id, menu_content);
            ctx.register_portal_with_layer(
                fission_core::PortalLayer::Flyout,
                Some(this.id),
                flyout_node,
            );
        }

        trigger
    }
}
