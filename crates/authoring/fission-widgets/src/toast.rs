use crate::stack::HStack;
use crate::Icon;
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonVariant, Container, Text, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use serde::{Deserialize, Serialize};

/// The severity level of a [`Toast`] notification, which determines the icon and color.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// A notification message with an icon, text, and close button.
///
/// Toasts are typically positioned at the top or bottom of the screen by the
/// application. The icon and color are determined by `kind`: Info (primary),
/// Success (check), Warning (orange triangle), or Error (red circle).
///
/// The toast renders with an elevated shadow and rounded corners. It does not
/// auto-dismiss -- the application must manage its lifecycle and remove it
/// when `on_close` fires or after a timeout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Toast {
    pub id: WidgetId,
    pub kind: ToastKind,
    pub message: String,
    pub on_close: Option<ActionEnvelope>,
}

impl From<Toast> for Widget {
    fn from(component: Toast) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;

        let (icon_path, icon_color) = match this.kind {
            ToastKind::Info => (material::action::info::regular(), tokens.colors.primary),
            ToastKind::Success => (
                material::action::check_circle::regular(),
                tokens.colors.on_background,
            ),
            ToastKind::Warning => (
                material::action::report_problem::regular(),
                Color {
                    r: 255,
                    g: 152,
                    b: 0,
                    a: 255,
                },
            ),
            ToastKind::Error => (material::alert::error::regular(), tokens.colors.error),
        };

        let content: Widget = HStack {
            spacing: Some(12.0),
            children: vec![
                Icon::svg(icon_path).color(icon_color).size(20.0).into(),
                Text::new(this.message.clone())
                    .color(tokens.colors.on_surface)
                    .flex_grow(1.0)
                    .into(),
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Icon::svg(material::navigation::close::regular())
                            .size(16.0)
                            .into(),
                    ),
                    on_press: this.on_close.clone(),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into();

        Container::new(content)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.medium)
            .shadow(
                tokens
                    .elevations
                    .level3
                    .unwrap_or(fission_core::op::BoxShadow {
                        color: Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 60,
                        },
                        blur_radius: 12.0,
                        offset: (0.0, 6.0),
                    }),
            )
            .padding_all(12.0)
            .into()
    }
}
