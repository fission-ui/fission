use crate::state::{increment, AppState, Increment};
use crate::title_bar::TitleBar;
use fission::prelude::*;

#[derive(Clone)]
pub struct App;

impl From<App> for Widget {
    fn from(_: App) -> Self {
        let (ctx, view) = fission::build::current::<AppState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: widgets![
                TitleBar,
                Container::new(Column {
                    gap: Some(tokens.spacing.l),
                    children: widgets![
                        Text::new(view.tr("window.instructions")),
                        Text::new(view.tr(if view.env().window.maximized == Some(true) {
                            "window.maximized"
                        } else {
                            "window.restored"
                        })),
                        Text::new(view.state().count.to_string()),
                        Button {
                            child: Some(Text::new(view.tr("window.increment")).into()),
                            on_press: Some(ctx.bind(Increment, reduce_with!(increment))),
                            ..Default::default()
                        }
                        .semantics_identifier("window.increment"),
                        if view.state().window_error {
                            Widget::from(Text::new(view.tr("window.error")))
                        } else {
                            Widget::from(Spacer::default())
                        },
                    ],
                    ..Default::default()
                })
                .padding_all(tokens.spacing.xl),
            ],
            ..Default::default()
        })
        .bg(tokens.colors.background)
        .width_length(Length::percent(100.0))
        .height_length(Length::percent(100.0))
        .into()
    }
}
