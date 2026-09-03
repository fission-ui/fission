use super::{composer_dialog_body::ComposerDialogBody, GalleryCase};
use crate::state::{
    close_composer, current_composition_atoms, AnimationGalleryState, CloseComposer,
};
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::{Modal, ModalAction, ModalMotion};

const DIALOG_WIDTH: f32 = 720.0;

pub struct CompositionDialog<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub case: &'a GalleryCase,
}

impl From<CompositionDialog<'_>> for Widget {
    fn from(dialog: CompositionDialog<'_>) -> Self {
        let close = dialog.ctx.bind(CloseComposer, reduce_with!(close_composer));

        Modal {
            id: WidgetId::explicit("animation-gallery.composer-dialog"),
            title: format!("Compose {}", dialog.case.title),
            content: ComposerDialogBody {
                ctx: dialog.ctx,
                state: dialog.state,
                case: dialog.case,
                atoms: current_composition_atoms(dialog.state),
            }
            .into(),
            is_open: dialog.state.composer_open,
            on_dismiss: Some(close.clone()),
            backdrop_semantics_identifier: None,
            close_semantics_identifier: Some("animation-gallery.composer.close".into()),
            surface_semantics_identifier: Some("animation-gallery.composer".into()),
            actions: vec![ModalAction {
                label: "Done".into(),
                on_press: Some(close),
                is_primary: true,
                semantics_identifier: Some("animation-gallery.composer.done".into()),
            }],
            width: Some(DIALOG_WIDTH),
            motion: Some(ModalMotion::Fade + ModalMotion::Scale),
        }
        .into()
    }
}
