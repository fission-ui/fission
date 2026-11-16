use fission::prelude::*;

use super::drag_log::DragLog;
use super::external_file_drop_target::ExternalFileDropTarget;
use crate::GalleryState;

fission_component! {
    pub(super) DragAuxiliaryColumn<GalleryState> {}
    view(_column, _state, cx) {} =>
        VStack(
            spacing: cx.view.env().theme.tokens.spacing.s,
            children: [ExternalFileDropTarget(), DragLog()],
        )
}
