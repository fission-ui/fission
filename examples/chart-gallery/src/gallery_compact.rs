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

        Column {
            id: Some(WidgetId::explicit("chart-gallery.layout.compact")),
            gap: Some(view.env().theme.tokens.spacing.m),
            flex_grow: 1.0,
            children: widgets![gallery.sidebar, gallery.content],
            ..Default::default()
        }
        .into()
    }
}
