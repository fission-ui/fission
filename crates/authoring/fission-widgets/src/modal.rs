use crate::stack::{HStack, VStack};
use crate::Icon;
use fission_core::op::Color;
use fission_core::ui::{
    Align, Button, ButtonVariant, Container, GestureDetector, Text, Widget, ZStack,
};
use fission_core::{ActionEnvelope, WidgetId};
use serde::{Deserialize, Serialize};

/// A modal dialog with a dimmed backdrop, title bar, content area, and action buttons.
///
/// When `is_open` is `true`, the modal renders as a centered card on a full-screen
/// semi-transparent backdrop. Tapping the backdrop dispatches `on_dismiss`. The modal
/// is rendered into the portal overlay layer (`PortalLayer::Modal`), so it appears
/// above all other content.
///
/// # Fields
///
/// * `id` - Stable widget identity for the portal system.
/// * `title` - Text displayed in the modal header.
/// * `content` - The main body content node.
/// * `is_open` - Controls visibility. When `false`, renders an invisible spacer.
/// * `on_dismiss` - Action dispatched when the backdrop or close button is tapped.
/// * `actions` - Footer buttons (e.g., Cancel, OK).
/// * `width` - Optional fixed width. Falls back to `ModalTheme::max_width` (600px).
///
/// # Example
///
/// ```rust,ignore
/// Modal {
///     id: WidgetId::explicit("confirm"),
///     title: "Delete item?".into(),
///     content: Text::new("This cannot be undone.").into(),
///     is_open: state.show_confirm,
///     on_dismiss: Some(dismiss_action),
///     actions: vec![
///         ModalAction { label: "Cancel".into(), on_press: Some(cancel), is_primary: false },
///         ModalAction { label: "Delete".into(), on_press: Some(delete), is_primary: true },
///     ],
///     width: None,
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Modal {
    pub id: WidgetId,
    pub title: String,
    pub content: Widget,
    pub is_open: bool,
    pub on_dismiss: Option<ActionEnvelope>,
    pub actions: Vec<ModalAction>,
    pub width: Option<f32>,
}

/// A single action button displayed in the modal footer.
///
/// When `is_primary` is `true`, the button uses `ButtonVariant::Filled` with
/// the primary color. Otherwise it uses `ButtonVariant::Outline`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalAction {
    pub label: String,
    pub on_press: Option<ActionEnvelope>,
    pub is_primary: bool,
}

impl From<Modal> for Widget {
    fn from(component: Modal) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        if !this.is_open {
            return fission_core::ui::widgets::spacer::Spacer::default().into();
        }

        let theme = &view.env().theme.components.modal;
        let tokens = &view.env().theme.tokens;
        let container_style = &theme.container_style;
        let viewport = view.viewport_size();
        let horizontal_margin = 24.0;
        let max_dialog_width = if viewport.width.is_finite() && viewport.width > 0.0 {
            (viewport.width - horizontal_margin * 2.0).max(280.0)
        } else {
            theme.max_width
        };
        let dialog_width = this.width.unwrap_or(theme.max_width).min(max_dialog_width);

        // Dimmed backdrop
        let backdrop =
            Container::new(fission_core::ui::widgets::spacer::Spacer::default())
                .bg_fill(theme.scrim_style.background.clone().unwrap_or(
                    fission_core::op::Fill::Solid(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 220,
                    }),
                ))
                .flex_grow(1.0)
                .into();

        let backdrop_btn = GestureDetector {
            on_tap: this.on_dismiss.clone(),
            child: backdrop,
            ..Default::default()
        }
        .into();

        // Modal Content
        let mut action_buttons = Vec::new();
        for action in &this.actions {
            action_buttons.push(
                Button {
                    variant: if action.is_primary {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::SecondaryGray
                    },
                    child: Some(
                        Text::new(action.label.clone())
                            .color(if action.is_primary {
                                tokens.colors.on_primary
                            } else {
                                tokens.colors.primary
                            })
                            .into(),
                    ),
                    on_press: action.on_press.clone(),
                    ..Default::default()
                }
                .into(),
            );
        }

        let mut modal_card_builder = Container::new(VStack {
            spacing: Some(16.0),
            children: vec![
                // Header
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Text::new(this.title.clone()).size(20.0).into(),
                        fission_core::ui::widgets::spacer::Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(
                                Icon::svg(fission_icons::material::navigation::close::regular())
                                    .size(20.0)
                                    .into(),
                            ),
                            on_press: this.on_dismiss.clone(),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
                // Content
                this.content.clone(),
                // Footer Actions
                HStack {
                    spacing: Some(8.0),
                    children: vec![fission_core::ui::widgets::spacer::Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into()]
                    .into_iter()
                    .chain(action_buttons)
                    .collect(),
                }
                .into(),
            ],
        })
        .bg_fill(
            container_style
                .background
                .clone()
                .unwrap_or(fission_core::op::Fill::Solid(theme.bg_color)),
        )
        .border_radius(container_style.radius.unwrap_or(theme.radius))
        .shadows(container_style.outer_shadows());

        if container_style.shadows.is_empty() {
            if let Some(s) = theme.shadow {
                modal_card_builder = modal_card_builder.shadow(s);
            }
        }

        let modal_card: Widget = modal_card_builder
            .width(dialog_width)
            .padding_all(24.0)
            .into();

        let center_layer = fission_core::ui::Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(Align::new(modal_card.clone()).into()),
            ..Default::default()
        }
        .into();

        let root = Container::new(ZStack {
            children: vec![
                // Full-screen backdrop button
                fission_core::ui::Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(backdrop_btn),
                    ..Default::default()
                }
                .into(),
                // Full-screen container with flex spacers to center the modal card
                center_layer,
            ],
            ..Default::default()
        })
        .flex_grow(1.0)
        .into();

        let positioned_root = fission_core::ui::Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(root),
            ..Default::default()
        }
        .into();
        ctx.register_portal_with_layer(
            fission_core::PortalLayer::Modal,
            Some(this.id),
            positioned_root,
        );

        fission_core::ui::widgets::spacer::Spacer::default().into()
    }
}
