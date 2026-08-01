use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use terminal_example::{TerminalExampleApp, TerminalExampleState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TerminalExample;

impl From<TerminalExample> for Widget {
    fn from(_component: TerminalExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<TerminalExampleState, _>::new(
            "showcase.example.terminal",
            view.state().preview_generation,
            TerminalExampleApp,
        )
        .with_state(terminal_example::embedded_state)
        .with_env(terminal_example::configure_embedded_env)
        .into()
    }
}
