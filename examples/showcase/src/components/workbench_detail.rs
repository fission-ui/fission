use super::{PreviewFrame, WorkbenchHeader};
use crate::catalog::ExampleDefinition;
use crate::state::ShowcaseState;
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct WorkbenchDetail {
    pub(super) example: ExampleDefinition,
}

impl From<WorkbenchDetail> for Widget {
    fn from(component: WorkbenchDetail) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        Column {
            children: widgets![
                Container::new(WorkbenchHeader {
                    example: component.example,
                })
                .padding_lengths(Length::all(Length::points(tokens.spacing.m))),
                PreviewFrame {
                    example: component.example,
                },
            ],
            gap: Some(tokens.spacing.none),
            flex_grow: 1.0,
            ..Default::default()
        }
        .into()
    }
}
