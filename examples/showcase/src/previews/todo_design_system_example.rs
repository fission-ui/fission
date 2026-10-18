use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use todo_design_system_example::{TodoApp, TodoState};

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) TodoDesignSystemExample<ShowcaseState> {}
    |_component, state, _cx| {} =>
        MountedExample::<TodoState, _>::new(
            "showcase.example.todo-design-system",
            state.preview_generation,
            TodoApp,
        )
        .with_state(todo_design_system_example::embedded_state)
        .with_env(todo_design_system_example::configure_embedded_env)
}
