use crate::foundations_section::ThemeBar;
use crate::pages::{PagePicker, PageView, Sidebar};
use crate::state::GalleryState;
use fission::prelude::*;

const PAGE_MAX_WIDTH: f32 = 880.0;

fission_component! {
    pub GalleryApp<GalleryState>
    |_app, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let page = state.page;
        // A phone-width window has no room for the sidebar beside the page.
        let compact = cx.view.viewport_size().width < tokens.breakpoints.compact_max;
        let page_padding = if compact {
            tokens.spacing.l
        } else {
            tokens.spacing.xl
        };

    } =>
    Container(Column(children: [
        if !state.embedded {
            Container(ThemeBar())
                .width_length(Length::percent(100.0))
                .flex_shrink(0.0)
        },
        if compact {
            Column(
                children: [
                    Container(PagePicker())
                        .width_length(Length::percent(100.0))
                        .padding_lengths(Length::symmetric(
                            Length::points(tokens.spacing.l),
                            Length::points(tokens.spacing.s),
                        ))
                        .bg(tokens.colors.surface)
                        .border_bottom(tokens.colors.border, tokens.sizing.border_hairline)
                        .flex_shrink(0.0),
                    GalleryPageScroll(page_padding, key: page.slug()),
                ],
                align_items: fission::op::AlignItems::Stretch,
                flex_grow: 1.0_f32,
                flex_shrink: 1.0_f32,
            )
        } else {
            Row(
                children: [
                    Sidebar(),
                    Container(Spacer())
                        .width(tokens.sizing.border_hairline)
                        .bg(tokens.colors.border)
                        .flex_shrink(0.0),
                    GalleryPageScroll(page_padding, key: page.slug()),
                ],
                align_items: fission::op::AlignItems::Stretch,
                flex_grow: 1.0_f32,
                flex_shrink: 1.0_f32,
            )
        },
    ]))
        .bg(tokens.colors.background)
        .width_length(Length::percent(100.0))
        .height_length(Length::percent(100.0))
}

fission_component! {
    GalleryPageScroll<GalleryState> { padding: f32 }
    |content, state, _cx| {} =>
        Scroll(
            id: WidgetId::explicit(&format!("gallery.page.{}", state.page.slug())),
            direction: FlexDirection::Column,
            child: Container(PageView())
                .width_length(Length::percent(100.0))
                .max_width_length(Length::points(PAGE_MAX_WIDTH))
                .padding_lengths(Length::all(Length::points(content.padding))),
            show_scrollbar: true,
            flex_grow: 1.0_f32,
            flex_shrink: 1.0_f32,
        )
}
