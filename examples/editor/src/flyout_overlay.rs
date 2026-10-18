use fission::prelude::*;

fission_component! {
/// A full-window layer holding `child` at `left`, `top`. A tap anywhere else
/// dispatches `on_dismiss`.
    pub(crate) FlyoutOverlay<()> {
        on_dismiss: ActionEnvelope,
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
                        on_tap: overlay.on_dismiss.clone(),
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
