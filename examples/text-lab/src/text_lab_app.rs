use crate::state::TextLabState;
use crate::text_lab_content::TextLabContent;
use crate::text_lab_modal::TextLabModal;
use fission::prelude::*;

const CONTENT_MIN_WIDTH: f32 = 280.0;
const CONTENT_MAX_WIDTH: f32 = 672.0;

fission_component! {
    pub TextLabApp<TextLabState>
    |_app, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SafeArea(
            id: None,
            child: Scroll(
                child: Container(
                        VStack(
                            spacing: tokens.spacing.none,
                            children: [TextLabContent, TextLabModal],
                        ),
                    )
                    .width_length(Length::clamp(
                        Length::points(CONTENT_MIN_WIDTH),
                        Length::percent(100.0),
                        Length::points(CONTENT_MAX_WIDTH),
                    ))
                    .padding_lengths(Length::all(Length::points(tokens.spacing.m))),
                show_scrollbar: true,
                flex_grow: 1.0_f32,
            ),
        )
}
