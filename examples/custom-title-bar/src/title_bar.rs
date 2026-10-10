use crate::state::{update_note, AppState, UpdateNote};
use crate::window_buttons::WindowButtons;
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;

pub struct TitleBar;

impl From<TitleBar> for Widget {
    fn from(_: TitleBar) -> Self {
        let (ctx, view) = fission::build::current::<AppState>();
        let tokens = &view.env().theme.tokens;
        WindowDragRegion::new(
            Container::new(Row {
                align_items: AlignItems::Center,
                wrap: FlexWrap::Wrap,
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
                    .min_width(tokens.sizing.control_md * 4.0),
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    },
                    WindowButtons,
                ],
                ..Default::default()
            })
            .width_length(Length::percent(100.0))
            .padding_all(tokens.spacing.s)
            .bg(tokens.colors.surface),
        )
        .semantics_identifier("window.drag")
        .into()
    }
}
