use crate::model::EditorState;
use fission::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub(crate) enum FlyoutDismiss {
    Menu,
    ContextMenu,
    Completions,
}

fission_component! {
/// A full-window layer holding `child` at `left`, `top`. A tap anywhere else
/// dispatches `on_dismiss`.
    pub(crate) FlyoutOverlay<EditorState> {
        dismiss: FlyoutDismiss,
        backdrop: Color,
        left: f32,
        top: f32,
        child: Widget,
    } |overlay, _state, _cx| =>
    FillParent(
        ZStack(
            children: [
                FillParent(
                    GestureDetector(
                        on_tap: |overlay, state, _cx| match overlay.dismiss {
                            FlyoutDismiss::Menu => state.active_menu = None,
                            FlyoutDismiss::ContextMenu => {
                                state.context_menu_visible = false;
                                state.context_menu_target = None;
                            }
                            FlyoutDismiss::Completions => state.close_completions(),
                        },
                        child: Container(Spacer())
                            .bg(overlay.backdrop)
                            .flex_grow(1.0),
                    ),
                ),
                Positioned(
                    left: Some(overlay.left),
                    top: Some(overlay.top),
                    child: overlay.child.clone(),
                ),
            ],
        ),
    )
}

fission_component! {
/// Stretches its child over every edge of the parent.
    pub(crate) FillParent<()> { child: Widget } |fill, _state, _cx| =>
        Positioned(
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: fill.child.clone(),
        )
}
