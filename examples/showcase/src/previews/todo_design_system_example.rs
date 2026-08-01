use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use todo_design_system_example::{TodoApp, TodoState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TodoDesignSystemExample;

impl From<TodoDesignSystemExample> for Widget {
    fn from(_component: TodoDesignSystemExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<TodoState, _>::new(
            "showcase.example.todo-design-system",
            view.state().preview_generation,
            TodoApp,
        )
        .with_env(todo_design_system_example::configure_embedded_env)
        .into()
    }
}
