use fission_core::op::{BoxShadow, Color};
use fission_core::ui::{Container, GestureDetector, Node, ZStack};
use fission_core::{
    ActionEnvelope, BuildCtx, View, Widget, WidgetNodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DrawerSide {
    Left,
    Right,
}

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
        // Animation logic
        // We want to animate the transform (TranslateX).
        // If open: 0. If closed: -width (Left) or +width (Right).
        // BUT we typically unmount the portal when closed.
        // To support exit animation, we need the portal to stay mounted but be off-screen?
        // Or we just snap for MVP.
        // Let's implement simple enter animation if is_open changes from false to true.
        // Since Widget `build` is stateless (re-run), we rely on `view` state.
        // But `is_open` is passed in.

        if !self.is_open {
            return fission_core::ui::widgets::Spacer::default().into_node();
        }

        let tokens = &view.env.theme.tokens;
        let width = self.width.unwrap_or(300.0);

        // Backdrop
        let backdrop = GestureDetector {
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

        // Drawer Content
        let content_node = Container::new(*self.content.clone())
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
            .into_node();

        let positioned_content = match self.side {
            DrawerSide::Left => fission_core::ui::Positioned {
                left: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                right: None,
                width: Some(width),
                child: Some(Box::new(content_node)),
                ..Default::default()
            },
            DrawerSide::Right => fission_core::ui::Positioned {
                right: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                left: None,
                width: Some(width),
                child: Some(Box::new(content_node)),
                ..Default::default()
            },
        }
        .into_node();

        // TODO: slide animation for drawer open/close

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
            child: Some(Box::new(root)),
            ..Default::default()
        }
        .into_node();
        ctx.register_portal_with_layer(fission_core::PortalLayer::Modal, Some(self.id), overlay_root);

        fission_core::ui::widgets::Spacer::default().into_node()
    }
}
