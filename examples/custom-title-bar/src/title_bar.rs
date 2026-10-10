use crate::state::{update_note, AppState, UpdateNote};
use crate::window_button::WindowButton;
use fission::op::AlignItems;
use fission::prelude::*;

pub struct TitleBar;

impl From<TitleBar> for Widget {
    fn from(_: TitleBar) -> Self {
        let (ctx, view) = fission::build::current::<AppState>();
        let tokens = &view.env().theme.tokens;
        let maximized = view.env().window.maximized.unwrap_or(false);
        WindowDragRegion::new(
            Container::new(Row {
                align_items: AlignItems::Center,
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Text::new(view.tr("window.title")),
                    Container::new(TextInput {
                        semantics_identifier: Some("window.note".into()),
                        label: Some(view.tr("window.note").into()),
                        value: view.state().note.clone(),
                        on_input: Some(ctx.bind(UpdateNote, reduce_with!(update_note))),
                        ..Default::default()
                    })
                    .flex_grow(1.0)
                    .min_width(0.0),
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    },
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
            })
            .padding_all(tokens.spacing.s)
            .bg(tokens.colors.surface),
        )
        .semantics_identifier("window.drag")
        .into()
    }
}
