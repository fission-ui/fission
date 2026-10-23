use fission::prelude::*;

use super::external_file_drop_panel::ExternalFileDropPanel;
use super::external_files_dropped;
use crate::GalleryState;

fission_component! {
    pub(super) ExternalFileDropTarget<GalleryState> {}
    |_target, _state, cx| {} =>
        DragTarget(
            id: WidgetId::explicit("gallery.drag.external_files"),
            semantics_identifier: "gallery.drag.external_files".to_string(),
            on_drop: |_target, state, cx| external_files_dropped(state, cx),
            child: ExternalFileDropPanel(false),
            hover_child: Some(ExternalFileDropPanel(true)),
        )
}
