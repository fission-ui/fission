use super::ThemeSwitch;
use crate::state::TodoState;
use fission::op::AlignItems;
use fission::prelude::*;

/// Below this width the theme switch moves under the title.
const STACKED_MAX_WIDTH: f32 = 520.0;

fission_component! {
    pub(super) TodoHeader<TodoState> |_header, _state, _cx| {} =>
        Responsive::new(HeaderLayout::new(false)).case(ResponsiveCase::max_width(
            STACKED_MAX_WIDTH,
            HeaderLayout::new(true),
        ))
}

fission_component! {
    HeaderLayout<TodoState> { stacked: bool } |layout, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        if layout.stacked {
            Column(
                gap: tokens.spacing.m,
                children: [
                    HeaderTitle(),
                    if !state.embedded { ThemeSwitch() },
                ],
            )
        } else {
            Row(
                gap: tokens.spacing.m,
                align_items: AlignItems::Start,
                children: [
                    HeaderTitle(),
                    if !state.embedded { ThemeSwitch() },
                ],
            )
        }
}

fission_component! {
    HeaderTitle<TodoState> |_title, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Column(
            gap: tokens.spacing.xs,
            flex_grow: 1.0_f32,
            children: [
                Text(cx.view.tr("todo.title"))
                    .size(typography.heading1_size)
                    .line_height(typography.heading1_size * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.heading),
                Text(cx.view.tr("todo.subtitle"))
                    .size(typography.body_medium_size)
                    .line_height(
                        typography.body_medium_size * typography.line_height_normal,
                    )
                    .color(tokens.colors.text_secondary),
            ],
        )
}
