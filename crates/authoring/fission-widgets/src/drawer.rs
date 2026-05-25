use fission_core::op::{BoxShadow, Color};
use fission_core::ui::{Container, GestureDetector, Node, ZStack};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget, WidgetNodeId};
use serde::{Deserialize, Serialize};

/// The edge from which a [`Drawer`] slides out.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
    pub id: WidgetNodeId,
    pub side: DrawerSide,
    pub is_open: bool,
    pub on_dismiss: Option<ActionEnvelope>,
    pub content: Box<Node>,
    pub width: Option<f32>,
}

impl<S: fission_core::AppState> Widget<S> for Drawer {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        if !self.is_open {
            return fission_core::ui::widgets::Spacer::default().into_node();
        }

        let tokens = &view.env.theme.tokens;
        let viewport = view.viewport_size();
        let max_panel_width = if viewport.width.is_finite() && viewport.width > 0.0 {
            (viewport.width - 24.0).max(180.0)
        } else {
            self.width.unwrap_or(300.0)
        };
        let width = self.width.unwrap_or(300.0).min(max_panel_width);

        // The drawer only mounts while open, so these are enter animations.
        // Exit animation would need a retained closing state owned above this
        // stateless widget.
        let backdrop_inner = GestureDetector {
            on_tap: self.on_dismiss.clone(),
            child: Box::new(
                Container::new(fission_core::ui::widgets::Spacer::default().into_node())
                    .bg(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 128,
                    })
                    .flex_grow(1.0)
                    .into_node(),
            ),
            ..Default::default()
        }
        .into_node();

        let backdrop_anim_id = WidgetNodeId::from_u128(self.id.as_u128() ^ 0xBACD_u128);
        ctx.anim_for(backdrop_anim_id)
            .request(fission_core::AnimationRequest {
                property: fission_core::AnimationPropertyId::Opacity,
                from: fission_core::AnimationStartValue::Explicit(0.0),
                to: 1.0,
                duration_ms: 200,
                repeat: false,
                delay_ms: 0,
                frame_interval_ms: None,
                easing: Default::default(),
            });
        let backdrop = fission_core::ui::Composite::new(backdrop_inner)
            .repaint_boundary(true)
            .animated_opacity(backdrop_anim_id, 0.0)
            .into_node();

        // Drawer Content
        let content_node = Container::new(*self.content.clone())
            .bg(tokens.colors.surface)
            .width(width)
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
            .into_node();

        let slide_anim_id = WidgetNodeId::from_u128(self.id.as_u128() ^ 0xD00D_u128);
        let slide_start = match self.side {
            DrawerSide::Left => -width,
            DrawerSide::Right => width,
        };
        // Start explicitly off-screen; relying on the current animation value
        // would make the first open frame snap to the final position.
        ctx.anim_for(slide_anim_id)
            .request(fission_core::AnimationRequest {
                property: fission_core::AnimationPropertyId::TranslateX,
                from: fission_core::AnimationStartValue::Explicit(slide_start),
                to: 0.0,
                duration_ms: 250,
                repeat: false,
                delay_ms: 0,
                frame_interval_ms: None,
                easing: Default::default(),
            });
        let animated_content = fission_core::ui::Composite::new(content_node)
            .repaint_boundary(true)
            .animated_translate_x(slide_anim_id, slide_start)
            .into_node();

        let positioned_content = match self.side {
            DrawerSide::Left => fission_core::ui::Positioned {
                left: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                right: None,
                width: Some(width),
                child: Some(Box::new(animated_content)),
                ..Default::default()
            },
            DrawerSide::Right => fission_core::ui::Positioned {
                right: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                left: None,
                width: Some(width),
                child: Some(Box::new(animated_content)),
                ..Default::default()
            },
        }
        .into_node();

        let root = ZStack {
            children: vec![
                fission_core::ui::Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(Box::new(backdrop)),
                    ..Default::default()
                }
                .into_node(),
                positioned_content,
            ],
            id: None,
        }
        .into_node();

        let overlay_root = fission_core::ui::Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(Box::new(
                fission_core::ui::widgets::FocusScope {
                    id: None,
                    is_barrier: true,
                    children: vec![root],
                }
                .into_node(),
            )),
            ..Default::default()
        }
        .into_node();
        ctx.register_portal_with_layer(
            fission_core::PortalLayer::Modal,
            Some(self.id),
            overlay_root,
        );

        fission_core::ui::widgets::Spacer::default().into_node()
    }
}
