use crate::state::GalleryState;
use fission::prelude::*;

fission_component! {
    pub(crate) GalleryCompact<GalleryState> { sidebar: Widget, content: Widget }
    |gallery, _state, cx| {} =>
        Scroll(
            id: WidgetId::explicit("chart-gallery.page-scroll.compact"),
            direction: FlexDirection::Column,
            child: Column(
                id: WidgetId::explicit("chart-gallery.layout.compact"),
                gap: cx.view.env().theme.tokens.spacing.m,
                children: [gallery.sidebar.clone(), gallery.content.clone()],
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
        )
}
