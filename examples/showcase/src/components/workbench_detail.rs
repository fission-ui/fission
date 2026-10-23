use super::{PreviewFrame, PreviewToolbar, WorkbenchHeader};
use crate::catalog::ExampleDefinition;
use crate::state::ShowcaseState;
use fission::prelude::*;

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(super) WorkbenchDetail<ShowcaseState> {
        example: ExampleDefinition,
    }
    |component, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Column(
            children: [
                Container(WorkbenchHeader(component.example))
                .padding_lengths(Length::symmetric(
                    Length::points(tokens.spacing.l),
                    Length::points(tokens.spacing.m),
                ))
                .bg(tokens.colors.surface),
                PreviewToolbar(),
                PreviewFrame(component.example),
            ],
            gap: tokens.spacing.none,
            flex_grow: 1.0,
        )
}
