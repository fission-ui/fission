use crate::stack::{HStack, VStack};
use crate::Icon;
use fission_core::op::Color;
use fission_core::ui::{
    Align, Button, ButtonVariant, Container, GestureDetector, Node, Text, ZStack,
};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget, WidgetNodeId};
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
///     id: WidgetNodeId::explicit("confirm"),
///     title: "Delete item?".into(),
///     content: Box::new(Text::new("This cannot be undone.").into_node()),
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
    pub id: WidgetNodeId,
    pub title: String,
    pub content: Box<Node>,
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

impl<S: fission_core::AppState> Widget<S> for Modal {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        if !self.is_open {
            return fission_core::ui::widgets::spacer::Spacer::default().into_node();
        }

        let theme = &view.env.theme.components.modal;
        let tokens = &view.env.theme.tokens;
        let viewport = view.viewport_size();
        let horizontal_margin = 24.0;
        let max_dialog_width = if viewport.width.is_finite() && viewport.width > 0.0 {
            (viewport.width - horizontal_margin * 2.0).max(280.0)
        } else {
            theme.max_width
        };
        let dialog_width = self.width.unwrap_or(theme.max_width).min(max_dialog_width);

        // Dimmed backdrop
        let backdrop =
            Container::new(fission_core::ui::widgets::spacer::Spacer::default().into_node())
                .bg(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 220,
                })
                .flex_grow(1.0)
                .into_node();

        let backdrop_btn = GestureDetector {
            on_tap: self.on_dismiss.clone(),
            child: Box::new(backdrop),
            ..Default::default()
        }
        .into_node();

        // Modal Content
        let mut action_buttons = Vec::new();
        for action in &self.actions {
            action_buttons.push(
                Button {
                    variant: if action.is_primary {
                        ButtonVariant::Filled
                    } else {
                        ButtonVariant::Outline
                    },
                    child: Some(Box::new(
                        Text::new(action.label.clone())
                            .color(if action.is_primary {
                                tokens.colors.on_primary
                            } else {
                                tokens.colors.primary
                            })
                            .into_node(),
                    )),
                    on_press: action.on_press.clone(),
                    ..Default::default()
                }
                .into_node(),
            );
        }

        let mut modal_card_builder =
            Container::new(
                VStack {
                    spacing: Some(16.0),
                    children: vec![
                        // Header
                        HStack {
                            spacing: Some(8.0),
                            children: vec![
                                Text::new(self.title.clone()).size(20.0).into_node(),
                                fission_core::ui::widgets::spacer::Spacer {
                                    flex_grow: 1.0,
                                    ..Default::default()
                                }
                                .into_node(),
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    child: Some(Box::new(
                                        Icon::svg(
                                            fission_icons::material::navigation::close::regular(),
                                        )
                                        .size(20.0)
                                        .into_node(),
                                    )),
                                    on_press: self.on_dismiss.clone(),
                                    ..Default::default()
                                }
                                .into_node(),
                            ],
                        }
                        .into_node(),
                        // Content
                        *self.content.clone(),
                        // Footer Actions
                        HStack {
                            spacing: Some(8.0),
                            children: vec![fission_core::ui::widgets::spacer::Spacer {
                                flex_grow: 1.0,
                                ..Default::default()
                            }
                            .into_node()]
                            .into_iter()
                            .chain(action_buttons)
                            .collect(),
                        }
                        .into_node(),
                    ],
                }
                .into_node(),
            )
            .bg(theme.bg_color)
            .border_radius(theme.radius);

        if let Some(s) = theme.shadow {
            modal_card_builder = modal_card_builder.shadow(s);
        }

        let modal_card = modal_card_builder
            .width(dialog_width)
            .padding_all(24.0)
            .into_node();

        let center_layer = fission_core::ui::Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(Box::new(Align::new(modal_card.clone()).into_node())),
            ..Default::default()
        }
        .into_node();

        let root = Container::new(
            ZStack {
                children: vec![
                    // Full-screen backdrop button
                    fission_core::ui::Positioned {
                        left: Some(0.0),
                        right: Some(0.0),
                        top: Some(0.0),
                        bottom: Some(0.0),
                        child: Some(Box::new(backdrop_btn)),
                        ..Default::default()
                    }
                    .into_node(),
                    // Full-screen container with flex spacers to center the modal card
                    center_layer,
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .flex_grow(1.0)
        .into_node();

        let positioned_root = fission_core::ui::Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(Box::new(root)),
            ..Default::default()
        }
        .into_node();
        ctx.register_portal_with_layer(
            fission_core::PortalLayer::Modal,
            Some(self.id),
            positioned_root,
        );

        fission_core::ui::widgets::spacer::Spacer::default().into_node()
    }
}
