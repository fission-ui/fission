use crate::state::CanvasExampleState;
use fission::prelude::*;

fission_component! {
    pub(crate) ExampleHeader<CanvasExampleState> |_header, _state, _cx| {} =>
        Responsive::new(HeaderContent::new(false)).case(ResponsiveCase::max_width(
            560.0,
            HeaderContent::new(true),
        ))
}

fission_component! {
    HeaderContent<CanvasExampleState> { compact: bool } |header, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.m,
            children: [
                Text("Interactive surfaces")
                    .size(if header.compact {
                        tokens.typography.heading2_size
                    } else {
                        tokens.typography.heading1_size
                    })
                    .weight(700)
                    .color(tokens.colors.text_primary),
                Text("One low-level camera primitive and one declarative node editor.")
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary),
            ],
        )
}
