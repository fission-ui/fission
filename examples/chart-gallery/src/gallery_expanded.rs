use crate::gallery_content::GalleryContent;
use crate::gallery_sidebar::GallerySidebar;
use fission::op::AlignItems;
use fission::prelude::*;

pub(crate) struct GalleryExpanded {
    pub sidebar: GallerySidebar,
    pub content: GalleryContent,
}

impl From<GalleryExpanded> for Widget {
    fn from(gallery: GalleryExpanded) -> Self {
        Row {
            id: Some(WidgetId::explicit("chart-gallery.layout.expanded")),
            align_items: AlignItems::Stretch,
            flex_grow: 1.0,
            children: widgets![
                gallery.sidebar,
                // One page scroll for the content, rather than a scroll area per card.
                Scroll {
                    id: Some(WidgetId::explicit("chart-gallery.content-scroll.expanded")),
                    direction: FlexDirection::Column,
                    child: Some(Widget::from(gallery.content)),
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
