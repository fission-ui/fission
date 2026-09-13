use super::ThemeSwitch;
use crate::state::TodoState;
use fission::op::AlignItems;
use fission::prelude::*;

/// Below this width the theme switch moves under the title.
const STACKED_MAX_WIDTH: f32 = 520.0;

pub(super) struct TodoHeader;

impl From<TodoHeader> for Widget {
    fn from(_header: TodoHeader) -> Self {
        Responsive::new(HeaderLayout { stacked: false })
            .case(ResponsiveCase::max_width(
                STACKED_MAX_WIDTH,
                HeaderLayout { stacked: true },
            ))
            .into()
    }
}

struct HeaderLayout {
    stacked: bool,
}

impl From<HeaderLayout> for Widget {
    fn from(layout: HeaderLayout) -> Self {
        let (_, view) = fission::build::current::<TodoState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let title = Column {
            gap: Some(tokens.spacing.xs),
            flex_grow: 1.0,
            children: widgets![
                Text::new(view.tr("todo.title"))
                    .size(typography.heading1_size)
                    .line_height(typography.heading1_size * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.heading),
                Text::new(view.tr("todo.subtitle"))
                    .size(typography.body_medium_size)
                    .line_height(typography.body_medium_size * typography.line_height_normal)
                    .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        };

        if layout.stacked {
            Column {
                gap: Some(tokens.spacing.m),
                children: widgets![title, ThemeSwitch],
                ..Default::default()
            }
            .into()
        } else {
            Row {
                gap: Some(tokens.spacing.m),
                align_items: AlignItems::Start,
                children: widgets![title, ThemeSwitch],
                ..Default::default()
            }
            .into()
        }
    }
}
