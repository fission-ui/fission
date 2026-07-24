use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use embed_video_example::{VideoEmbedApp, VideoEmbedState};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EmbedVideoExample;

impl From<EmbedVideoExample> for Widget {
    fn from(_component: EmbedVideoExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<VideoEmbedState, _>::new(
            "showcase.example.embed-video",
            view.state().preview_generation,
            VideoEmbedApp,
        )
        .into()
    }
}
