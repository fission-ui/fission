use fission::prelude::*;

use super::external_file_drop_panel::ExternalFileDropPanel;
use super::{external_files_dropped, ExternalFilesDropped};
use crate::GalleryState;

fission_component! {
    pub(super) ExternalFileDropTarget<GalleryState> {}
    |_target, _state, cx| {} =>
        DragTarget {
            id: Some(WidgetId::explicit("gallery.drag.external_files")),
            semantics_identifier: Some("gallery.drag.external_files".into()),
            on_drop: Some(cx.build.bind(ExternalFilesDropped, reduce_with!(external_files_dropped))),
            child: ExternalFileDropPanel::new(false).into(),
            hover_child: Some(ExternalFileDropPanel::new(true).into()),
        }
}
