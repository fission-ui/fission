use crate::state::GalleryState;
use fission::op::AlignItems;
use fission::prelude::*;

fission_component! {
    pub(crate) GalleryExpanded<GalleryState> { sidebar: Widget, content: Widget }
    |gallery, _state, _cx| {} =>
        Row(
            id: WidgetId::explicit("chart-gallery.layout.expanded"),
            align_items: AlignItems::Stretch,
            flex_grow: 1.0,
            children: [
                gallery.sidebar.clone(),
                Scroll(
                    id: WidgetId::explicit("chart-gallery.content-scroll.expanded"),
                    direction: FlexDirection::Column,
                    child: Some(gallery.content.clone()),
                    show_scrollbar: true,
                    flex_grow: 1.0,
                ),
            ],
        )
}
