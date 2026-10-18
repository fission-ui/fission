use crate::router::ShowcaseRouter;
use crate::state::ShowcaseState;
use fission::prelude::*;

fission_component! {
    pub(crate) ShowcaseApp<ShowcaseState>
    |_app, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(ShowcaseRouter)
        .width_length(Length::vw(100.0))
        .height_length(Length::vh(100.0))
        .bg(tokens.colors.background)
}
