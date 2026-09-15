use crate::foundations_section::ThemeBar;
use crate::pages::{PageView, Sidebar};
use crate::state::GalleryState;
use fission::prelude::*;

const PAGE_MAX_WIDTH: f32 = 880.0;

#[derive(Clone)]
pub struct GalleryApp;

impl From<GalleryApp> for Widget {
    fn from(_app: GalleryApp) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let page = view.state().page;

        // Each page keeps its own scroll position, so a newly opened page starts at the top.
        let content: Widget = Scroll {
            id: Some(WidgetId::explicit(&format!("gallery.page.{}", page.slug()))),
            direction: FlexDirection::Column,
            child: Some(
                Container::new(PageView)
                    .width_length(Length::percent(100.0))
                    .max_width_length(Length::points(PAGE_MAX_WIDTH))
                    .padding_lengths(Length::all(Length::points(tokens.spacing.xl)))
                    .into(),
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into();

        let divider: Widget = Container::new(Spacer::default())
            .width(tokens.sizing.border_hairline)
            .bg(tokens.colors.border)
            .flex_shrink(0.0)
            .into();
        let body: Widget = Row {
            children: widgets![Sidebar, divider, content],
            align_items: fission::op::AlignItems::Stretch,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into();

        // The theme bar sits above the sidebar and page so it stays in reach everywhere.
        // It must not shrink: a shrinking column squeezed it to a sliver.
        let theme_bar: Widget = Container::new(ThemeBar)
            .width_length(Length::percent(100.0))
            .flex_shrink(0.0)
            .into();
        let shell: Widget = Column {
            children: widgets![theme_bar, body],
            ..Default::default()
        }
        .into();

        // The page ground comes from the theme, so dark mode and other looks paint it.
        Container::new(shell)
            .bg(tokens.colors.background)
            .width_length(Length::percent(100.0))
            .height_length(Length::percent(100.0))
            .into()
    }
}
