use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use editor_example::{EditorApp, EditorState};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EditorExample;

impl From<EditorExample> for Widget {
    fn from(_component: EditorExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<EditorState, _>::new(
            "showcase.example.editor",
            view.state().preview_generation,
            EditorApp,
        )
        .with_state(editor_example::embedded_state)
        .with_env(editor_example::configure_embedded_env)
        .into()
    }
}
