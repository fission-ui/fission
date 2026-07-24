use fission::prelude::*;

use super::external_file_drop_panel::ExternalFileDropPanel;
use super::{external_files_dropped, ExternalFilesDropped};
use crate::GalleryState;

pub(super) struct ExternalFileDropTarget;

impl From<ExternalFileDropTarget> for Widget {
    fn from(_target: ExternalFileDropTarget) -> Self {
        let (ctx, _) = fission::build::current::<GalleryState>();

        DragTarget {
            id: Some(WidgetId::explicit("gallery.drag.external_files")),
            semantics_identifier: Some("gallery.drag.external_files".into()),
            on_drop: Some(ctx.bind(ExternalFilesDropped, reduce_with!(external_files_dropped))),
            child: ExternalFileDropPanel { hovered: false }.into(),
            hover_child: Some(ExternalFileDropPanel { hovered: true }.into()),
        }
        .into()
    }
}
