use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use field_inspector_example::{FieldInspectorApp, FieldInspectorState};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FieldInspectorExample;

impl From<FieldInspectorExample> for Widget {
    fn from(_component: FieldInspectorExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<FieldInspectorState, _>::new(
            "showcase.example.field-inspector",
            view.state().preview_generation,
            FieldInspectorApp,
        )
        .into()
    }
}
