use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use text_lab_example::{TextLabApp, TextLabState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextLabExample;

impl From<TextLabExample> for Widget {
    fn from(_component: TextLabExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<TextLabState, _>::new(
            "showcase.example.text-lab",
            view.state().preview_generation,
            TextLabApp,
        )
        .into()
    }
}
