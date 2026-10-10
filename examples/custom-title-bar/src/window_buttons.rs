use crate::state::AppState;
use crate::window_button::WindowButton;
use fission::op::JustifyContent;
use fission::prelude::*;

pub struct WindowButtons;

impl From<WindowButtons> for Widget {
    fn from(_: WindowButtons) -> Self {
        let (_, view) = fission::build::current::<AppState>();
        let maximized = view.env().window.maximized.unwrap_or(false);
        Row {
            flex_grow: 1.0,
            justify_content: JustifyContent::End,
            gap: Some(view.env().theme.tokens.spacing.s),
            children: widgets![
                WindowButton {
                    command: WindowCommand::Minimize,
                    label_key: "window.minimize",
                    identifier: "window.minimize"
                },
                WindowButton {
                    command: if maximized {
                        WindowCommand::Restore
                    } else {
                        WindowCommand::Maximize
                    },
                    label_key: if maximized {
                        "window.restore"
                    } else {
                        "window.maximize"
                    },
                    identifier: "window.maximize",
                },
                WindowButton {
                    command: WindowCommand::Close,
                    label_key: "window.close",
                    identifier: "window.close"
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
