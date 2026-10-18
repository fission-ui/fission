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
use fission::prelude::*;
use general::GeneralSettings;
use labels::LabelSettings;
use labs::LabsSettings;
use section::SectionDivider;
use signature::SignatureSettings;
use tips::TipsSettings;

fission_component! {
    pub SettingsModal<InboxState> |_modal, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let viewport = cx.view.viewport_size();
    } =>
    Modal(
        id: WidgetId::explicit("settings_modal"),
        title: cx.tr("settings.title"),
        is_open: true,
        on_dismiss: cx.bind(SetSettingsOpen(false), reduce_with!(set_settings_open)),
        backdrop_semantics_identifier: Some("settings.modal.backdrop".into()),
        close_semantics_identifier: Some("settings.modal.close".into()),
        surface_semantics_identifier: Some("settings.modal.surface".into()),
        width: (viewport.width.max(0.0) - tokens.spacing.xxl).clamp(360.0, 640.0),
        content: Scroll(
                direction: FlexDirection::Column,
                height: (viewport.height.max(0.0) - 180.0).clamp(320.0, 560.0),
                show_scrollbar: true,
                child: VStack(
                        spacing: tokens.spacing.m,
                        children: [
                            GeneralSettings(),
                            SectionDivider(),
                            AppearanceSettings(),
                            SectionDivider(),
                            SignatureSettings(),
                            SectionDivider(),
                            LabsSettings(),
                            LabelSettings(),
                            TipsSettings(),
                        ],
                ),
            ),
            actions: vec![ModalAction {
                label: cx.tr("settings.modal.done_label"),
                is_primary: true,
                on_press: Some(cx.bind(SetSettingsOpen(false), reduce_with!(set_settings_open))),
                semantics_identifier: Some("settings.modal.done".into()),
            }],
            motion: ModalMotion::Default,
        )
}
