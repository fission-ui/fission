use crate::stack::HStack;
use crate::{flyout, Icon, Menu, MenuItem};
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Text, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use serde::{Deserialize, Serialize};

/// A single option in a [`Select`] dropdown.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectItem {
    pub label: String,
    pub icon: Option<String>,
    pub on_select: ActionEnvelope,
}

/// A dropdown selector that displays the selected label and opens a [`Menu`] flyout.
///
/// Renders an outline button showing the current selection (or a placeholder).
/// When `is_open` is `true`, a scrollable menu of [`SelectItem`] entries appears
/// anchored to the button via the flyout portal system.
///
/// # Example
///
/// ```rust,ignore
/// Select {
///     id: WidgetId::explicit("country"),
///     selected_label: Some("United States".into()),
///     items: vec![
///         SelectItem { label: "United States".into(), icon: None, on_select: us_action },
///         SelectItem { label: "Canada".into(), icon: None, on_select: ca_action },
///     ],
///     is_open: state.country_open,
///     on_toggle: Some(toggle_action),
///     placeholder: "Choose country...".into(),
///     width: Some(250.0),
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Select {
    pub id: WidgetId,
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
            id: WidgetId::explicit("select"),
            selected_label: None,
            items: Vec::new(),
            is_open: false,
            on_toggle: None,
            placeholder: "Select...".into(),
            width: Some(200.0),
        }
    }
}

impl From<Select> for Widget {
    fn from(component: Select) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let anchor_id = WidgetId::derived(this.id.as_u128(), &[]);

        let display_label = this.selected_label.as_deref().unwrap_or(&this.placeholder);
        let label_color = if this.selected_label.is_some() {
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
                    .into(),
                // Spacer to push chevron to the right
                fission_core::ui::widgets::spacer::Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Icon::svg(material::navigation::expand_more::regular())
                    .size(20.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            ],
        }
        .into();

        let trigger = Button {
            id: Some(anchor_id.into()),
            variant: ButtonVariant::Outline,
            content_align: ButtonContentAlign::Start,
            child: Some(trigger_content),
            on_press: this.on_toggle.clone(),
            width: this.width,
            ..Default::default()
        }
        .into();

        if this.is_open {
            let menu_items = this
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
                width: this.width,
                max_height: Some(300.0),
            }
            .into();

            let flyout_node = flyout(anchor_id, menu);
            ctx.register_portal_with_layer(
                fission_core::PortalLayer::Flyout,
                Some(this.id),
                flyout_node,
            );
        }

        trigger
    }
}
