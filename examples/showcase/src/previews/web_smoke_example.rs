use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use web_smoke_example::{CounterApp, CounterState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct WebSmokeExample;

impl From<WebSmokeExample> for Widget {
    fn from(_component: WebSmokeExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<CounterState, _>::new(
            "showcase.example.web-smoke",
            view.state().preview_generation,
            CounterApp,
        )
        .into()
    }
}
