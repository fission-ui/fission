use super::section::SectionHeading;
use crate::model::InboxState;
use fission::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
enum LabSetting {
    SmartCompose,
    Offline,
    AutoAdvance,
}

fission_component! {
/// Experimental features the reader can switch on.
    pub(super) LabsSettings<InboxState> |_settings, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        VStack(
            spacing: tokens.spacing.m,
            children: [
                SectionHeading("settings.labs.title"),
                Card(
                    child: VStack(
                        spacing: tokens.spacing.s,
                        children: [
                            LabToggle(
                                Icon::svg(material::action::check_circle::regular()),
                                "settings.labs.smart_compose",
                                state.smart_compose_enabled,
                                LabSetting::SmartCompose,
                            ),
                            LabToggle(
                                Icon::svg(material::action::report_problem::regular()),
                                "settings.labs.offline",
                                state.offline_enabled,
                                LabSetting::Offline,
                            ),
                            LabToggle(
                                Icon::svg(material::action::info::regular()),
                                "settings.labs.auto_advance",
                                state.auto_advance_enabled,
                                LabSetting::AutoAdvance,
                            ),
                        ],
                    ),
                ),
            ],
        )
}

fission_component! {
/// One experimental feature with its switch.
    LabToggle<InboxState> {
        icon: Icon,
        label_key: &'static str,
        checked: bool,
        setting: LabSetting,
    } |toggle, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        HStack(
            spacing: tokens.spacing.s,
            children: [
                toggle.icon.clone().size(tokens.typography.font_size_lg),
                Text(TextContent::Key(toggle.label_key.into())),
                Spacer(flex_grow: 1.0),
                Switch(
                    checked: toggle.checked,
                    on_toggle: |toggle, state, _cx| match toggle.setting {
                        LabSetting::SmartCompose => {
                            state.smart_compose_enabled = !state.smart_compose_enabled;
                        }
                        LabSetting::Offline => state.offline_enabled = !state.offline_enabled,
                        LabSetting::AutoAdvance => {
                            state.auto_advance_enabled = !state.auto_advance_enabled;
                        }
                    },
                ),
            ],
        )
}
