use fission::prelude::*;

use super::drag_log::DragLog;
use super::external_file_drop_target::ExternalFileDropTarget;
use crate::GalleryState;

pub(super) struct DragAuxiliaryColumn;

impl From<DragAuxiliaryColumn> for Widget {
    fn from(_column: DragAuxiliaryColumn) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();

        VStack {
            spacing: Some(view.env().theme.tokens.spacing.s),
            children: widgets![ExternalFileDropTarget, DragLog],
        }
        .into()
    }
}
