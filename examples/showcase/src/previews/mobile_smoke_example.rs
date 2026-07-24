use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use mobile_smoke_example::{MobileSmokeApp, SmokeState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct MobileSmokeExample;

impl From<MobileSmokeExample> for Widget {
    fn from(_component: MobileSmokeExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<SmokeState, _>::new(
            "showcase.example.mobile-smoke",
            view.state().preview_generation,
            MobileSmokeApp,
        )
        .into()
    }
}
