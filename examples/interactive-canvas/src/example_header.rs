use crate::state::CanvasExampleState;
use fission::prelude::*;

pub(crate) struct ExampleHeader;

impl From<ExampleHeader> for Widget {
    fn from(_header: ExampleHeader) -> Self {
        Responsive::new(HeaderContent { compact: false })
            .case(ResponsiveCase::max_width(
                560.0,
                HeaderContent { compact: true },
            ))
            .into()
    }
}

struct HeaderContent {
    compact: bool,
}

impl From<HeaderContent> for Widget {
    fn from(header: HeaderContent) -> Self {
        let (_, view) = fission::build::current::<CanvasExampleState>();
        let tokens = &view.env().theme.tokens;
        Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                Text::new("Interactive surfaces")
                    .size(if header.compact {
                        tokens.typography.heading2_size
                    } else {
                        tokens.typography.heading1_size
                    })
                    .weight(700)
                    .color(tokens.colors.text_primary),
                Text::new("One low-level camera primitive and one declarative node editor.")
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        }
        .into()
    }
}
