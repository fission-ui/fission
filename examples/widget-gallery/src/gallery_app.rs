use crate::colour_picker_section::ColourPickerSection;
use crate::data_section::DataSection;
use crate::display_section::DisplaySection;
use crate::drag_drop::DragDropSection;
use crate::feedback_section::FeedbackSection;
use crate::gallery_header::GalleryHeader;
use crate::input_section::InputSection;
use crate::navigation_section::NavigationSection;
use crate::overlay_section::OverlaySection;
use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{Center, Spacer, VStack};

const GALLERY_MAX_WIDTH: f32 = 1040.0;

#[derive(Clone)]
pub struct GalleryApp;

impl From<GalleryApp> for Widget {
    fn from(_app: GalleryApp) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;

        Scroll {
            direction: FlexDirection::Column,
            child: Some(
                Center {
                    child: Container::new(VStack {
                        spacing: Some(tokens.spacing.l),
                        children: widgets![
                            GalleryHeader,
                            DisplaySection,
                            InputSection,
                            ColourPickerSection,
                            FeedbackSection,
                            NavigationSection,
                            DataSection,
                            DragDropSection,
                            OverlaySection,
                            Spacer {
                                height: Some(tokens.spacing.xl),
                                ..Default::default()
                            },
                        ],
                    })
                    .width_length(Length::percent(100.0))
                    .max_width_length(Length::points(GALLERY_MAX_WIDTH))
                    .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
                    .into(),
                }
                .into(),
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()
    }
}
