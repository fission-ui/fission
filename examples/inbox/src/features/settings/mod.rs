//! The settings modal, composed from one widget per section.

mod appearance;
mod general;
mod labels;
mod labs;
mod section;
mod signature;
mod tips;

use crate::model::settings::set_settings_open;
use crate::model::{InboxState, SetSettingsOpen};
use appearance::AppearanceSettings;
use fission::core::ui::{Scroll, Widget};
use fission::core::{reduce_with, FlexDirection, WidgetId};
use fission::widgets::{Modal, ModalAction, ModalMotion, VStack};
use general::GeneralSettings;
use labels::LabelSettings;
use labs::LabsSettings;
use section::SectionDivider;
use signature::SignatureSettings;
use tips::TipsSettings;

pub struct SettingsModal;

impl From<SettingsModal> for Widget {
    fn from(_component: SettingsModal) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let viewport = view.viewport_size();

        Modal {
            id: WidgetId::explicit("settings_modal"),
            title: view.tr("settings.title"),
            is_open: true,
            on_dismiss: Some(ctx.bind(SetSettingsOpen(false), reduce_with!(set_settings_open))),
            backdrop_semantics_identifier: Some("settings.modal.backdrop".into()),
            close_semantics_identifier: Some("settings.modal.close".into()),
            surface_semantics_identifier: Some("settings.modal.surface".into()),
            width: Some((viewport.width.max(0.0) - tokens.spacing.xxl).clamp(360.0, 640.0)),
            content: Scroll {
                direction: FlexDirection::Column,
                width: None,
                height: Some((viewport.height.max(0.0) - 180.0).clamp(320.0, 560.0)),
                show_scrollbar: true,
                child: Some(
                    VStack {
                        spacing: Some(tokens.spacing.m),
                        children: vec![
                            GeneralSettings.into(),
                            SectionDivider.into(),
                            AppearanceSettings.into(),
                            SectionDivider.into(),
                            SignatureSettings.into(),
                            SectionDivider.into(),
                            LabsSettings.into(),
                            LabelSettings.into(),
                            TipsSettings.into(),
                        ],
                    }
                    .into(),
                ),
                ..Default::default()
            }
            .into(),
            actions: vec![ModalAction {
                label: view.tr("settings.modal.done_label"),
                is_primary: true,
                on_press: Some(ctx.bind(SetSettingsOpen(false), reduce_with!(set_settings_open))),
                semantics_identifier: Some("settings.modal.done".into()),
            }],
            motion: Some(ModalMotion::Default),
        }
        .into()
    }
}
