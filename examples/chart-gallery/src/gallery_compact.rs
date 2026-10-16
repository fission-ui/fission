use crate::gallery_content::GalleryContent;
use crate::gallery_sidebar::GallerySidebar;
use crate::state::GalleryState;
use fission::prelude::*;

pub(crate) struct GalleryCompact {
    pub sidebar: GallerySidebar,
    pub content: GalleryContent,
}

impl From<GalleryCompact> for Widget {
    fn from(gallery: GalleryCompact) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();

        // The picker and the content scroll together as one page, so nothing is
        // trapped in a nested scroll area.
        Scroll {
            id: Some(WidgetId::explicit("chart-gallery.page-scroll.compact")),
            direction: FlexDirection::Column,
            child: Some(
                Column {
                    id: Some(WidgetId::explicit("chart-gallery.layout.compact")),
                    gap: Some(view.env().theme.tokens.spacing.m),
                    children: widgets![gallery.sidebar, gallery.content],
                    ..Default::default()
                }
                .into(),
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into()
    }
}
