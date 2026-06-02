use fission_core::op::{BoxShadow, Color};
use fission_core::ui::{Container, GestureDetector, Widget, ZStack};
use fission_core::{ActionEnvelope, WidgetId};
use serde::{Deserialize, Serialize};

/// The edge from which a [`Drawer`] slides out.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DrawerSide {
    Left,
    Right,
}

/// A slide-out panel from the left or right edge of the screen.
///
/// When `is_open` is `true`, the drawer renders as a portal overlay with a
/// semi-transparent backdrop and a fixed-width panel positioned against the
/// specified `side`. Tapping the backdrop dispatches `on_dismiss`.
///
/// # Fields
///
/// * `side` - `Left` or `Right` edge.
/// * `width` - Panel width in logical pixels (default 300).
/// * `is_open` - Controls visibility.
/// * `on_dismiss` - Action dispatched when the backdrop is tapped.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Drawer {
    pub id: WidgetId,
    pub side: DrawerSide,
    pub is_open: bool,
    pub on_dismiss: Option<ActionEnvelope>,
    pub content: Widget,
    pub width: Option<f32>,
}

impl From<Drawer> for Widget {
    fn from(component: Drawer) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        // Animation logic
        // We want to animate the transform (TranslateX).
        // If open: 0. If closed: -width (Left) or +width (Right).
        // BUT we typically unmount the portal when closed.
        // To support exit animation, we need the portal to stay mounted but be off-screen?
        // Or we just snap for MVP.
        // Let's implement simple enter animation if is_open changes from false to true.
        // Since Widget `build` is stateless (re-run), we rely on `view` state.
        // But `is_open` is passed in.

        if !this.is_open {
            return fission_core::ui::widgets::Spacer::default().into();
        }

        let tokens = &view.env().theme.tokens;
        let viewport = view.viewport_size();
        let max_panel_width = if viewport.width.is_finite() && viewport.width > 0.0 {
            (viewport.width - 24.0).max(180.0)
        } else {
            this.width.unwrap_or(300.0)
        };
        let width = this.width.unwrap_or(300.0).min(max_panel_width);

        // Backdrop
        let backdrop = GestureDetector {
            on_tap: this.on_dismiss.clone(),
            child: Container::new(fission_core::ui::widgets::Spacer::default())
                .bg(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 128,
                })
                .flex_grow(1.0)
                .into(),
            ..Default::default()
        }
        .into();

        // Drawer Content
        let content_node = Container::new(this.content.clone())
            .bg(tokens.colors.surface)
            .width(width)
            // Height fills parent (Positioned top/bottom 0)
            .shadow(tokens.elevations.level3.unwrap_or(BoxShadow {
                color: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 60,
                },
                blur_radius: 16.0,
                offset: (0.0, 0.0),
            }))
            .padding_all(0.0)
            .into();

        let positioned_content = match this.side {
            DrawerSide::Left => fission_core::ui::Positioned {
                left: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                right: None,
                width: Some(width),
                child: Some(content_node),
                ..Default::default()
            },
            DrawerSide::Right => fission_core::ui::Positioned {
                right: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                left: None,
                width: Some(width),
                child: Some(content_node),
                ..Default::default()
            },
        }
        .into();

        // TODO: slide animation for drawer open/close

        let root = ZStack {
            children: vec![
                fission_core::ui::Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(backdrop),
                    ..Default::default()
                }
                .into(),
                positioned_content,
            ],
            id: None,
        }
        .into();

        let overlay_root = fission_core::ui::Positioned {
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
            overlay_root,
        );

        fission_core::ui::widgets::Spacer::default().into()
    }
}
