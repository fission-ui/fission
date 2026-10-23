use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use text_lab_example::{TextLabApp, TextLabState};

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) TextLabExample<ShowcaseState> {}
    |_component, state, _cx| {} =>
        MountedExample::<TextLabState, _>::new(
            "showcase.example.text-lab",
            state.preview_generation,
            TextLabApp,
        )
}
