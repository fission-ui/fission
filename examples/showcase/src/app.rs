use crate::router::ShowcaseRouter;
use crate::state::ShowcaseState;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct ShowcaseApp;

impl From<ShowcaseApp> for Widget {
    fn from(_component: ShowcaseApp) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;

        Container::new(ShowcaseRouter)
            .width_length(Length::vw(100.0))
            .height_length(Length::vh(100.0))
            .bg(tokens.colors.background)
            .into()
    }
}
