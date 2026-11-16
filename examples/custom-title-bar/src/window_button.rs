use crate::state::{control_window, AppState, ControlWindow};
use fission::prelude::*;

pub struct WindowButton {
    pub command: WindowCommand,
    pub label_key: &'static str,
    pub identifier: &'static str,
}

impl From<WindowButton> for Widget {
    fn from(button: WindowButton) -> Self {
        let (ctx, view) = fission::build::current::<AppState>();
        Button {
            child: Some(Text::new(view.tr(button.label_key)).into()),
            on_press: Some(ctx.bind(ControlWindow(button.command), reduce_with!(control_window))),
            ..Default::default()
        }
        .semantics_identifier(button.identifier)
        .into()
    }
}
