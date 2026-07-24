use crate::state::TextLabState;
use crate::text_lab_content::TextLabContent;
use crate::text_lab_modal::TextLabModal;
use fission::prelude::*;

const CONTENT_MIN_WIDTH: f32 = 280.0;
const CONTENT_MAX_WIDTH: f32 = 672.0;

#[derive(Clone)]
pub(crate) struct TextLabApp;

impl From<TextLabApp> for Widget {
    fn from(_app: TextLabApp) -> Self {
        let (_, view) = fission::build::current::<TextLabState>();
        let tokens = &view.env().theme.tokens;

        SafeArea {
            id: None,
            child: Scroll {
                child: Some(
                    Container::new(VStack {
                        spacing: Some(tokens.spacing.none),
                        children: widgets![TextLabContent, TextLabModal],
                    })
                    .width_length(Length::clamp(
                        Length::points(CONTENT_MIN_WIDTH),
                        Length::percent(100.0),
                        Length::points(CONTENT_MAX_WIDTH),
                    ))
                    .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
                    .into(),
                ),
                show_scrollbar: true,
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}
