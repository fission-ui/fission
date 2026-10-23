use super::{composer_dialog_body::ComposerDialogBody, GalleryCase};
use crate::state::{
    close_composer, current_composition_atoms, AnimationGalleryState, CloseComposer,
};
use fission::prelude::*;
use fission::widgets::{Modal, ModalAction, ModalMotion};

const DIALOG_WIDTH: f32 = 720.0;

fission_component! {
    pub CompositionDialog<AnimationGalleryState> { case: GalleryCase }
    |dialog, state, cx| {
        let close = cx.bind(CloseComposer, reduce_with!(close_composer));
    } =>
        Modal(
            id: WidgetId::explicit("animation-gallery.composer-dialog"),
            title: format!("Compose {}", dialog.case.title),
            content: ComposerDialogBody(dialog.case, current_composition_atoms(state).to_vec()),
            is_open: state.composer_open,
            on_dismiss: close.clone(),
            close_semantics_identifier: "animation-gallery.composer.close".to_string(),
            surface_semantics_identifier: "animation-gallery.composer".to_string(),
            actions: vec![ModalAction {
                label: "Done".into(),
                on_press: Some(close.clone()),
                is_primary: true,
                semantics_identifier: Some("animation-gallery.composer.done".into()),
            }],
            width: DIALOG_WIDTH,
            motion: ModalMotion::Fade + ModalMotion::Scale,
        )
}
