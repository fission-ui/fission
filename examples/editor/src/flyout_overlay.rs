use fission::core::op::Color;
use fission::core::ui::{Container, GestureDetector, Positioned, Widget, ZStack};
use fission::core::ActionEnvelope;
use fission::widgets::Spacer;

/// A full-window layer holding `child` at `left`, `top`. A tap anywhere else
/// dispatches `on_dismiss`.
pub(crate) struct FlyoutOverlay {
    pub on_dismiss: ActionEnvelope,
    pub backdrop: Color,
    pub left: f32,
    pub top: f32,
    pub child: Widget,
}

impl From<FlyoutOverlay> for Widget {
    fn from(overlay: FlyoutOverlay) -> Self {
        let backdrop = GestureDetector {
            on_tap: Some(overlay.on_dismiss),
            child: Container::new(Spacer::default())
                .bg(overlay.backdrop)
                .flex_grow(1.0)
                .into(),
            ..Default::default()
        };
        let layer = ZStack {
            children: vec![
                FillParent(backdrop.into()).into(),
                Positioned {
                    left: Some(overlay.left),
                    top: Some(overlay.top),
                    child: Some(overlay.child),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        };
        FillParent(layer.into()).into()
    }
}

/// Stretches its child over every edge of the parent.
pub(crate) struct FillParent(pub Widget);

impl From<FillParent> for Widget {
    fn from(fill: FillParent) -> Self {
        Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(fill.0),
            ..Default::default()
        }
        .into()
    }
}
