use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use embed_3d_example::{Scene3DEmbedApp, Scene3DEmbedState};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Embed3dExample;

impl From<Embed3dExample> for Widget {
    fn from(_component: Embed3dExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<Scene3DEmbedState, _>::new(
            "showcase.example.embed-3d",
            view.state().preview_generation,
            Scene3DEmbedApp,
        )
        .into()
    }
}
