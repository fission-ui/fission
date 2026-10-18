use super::section::SectionHeading;
use crate::model::settings::{
    set_auto_advance_enabled, set_offline_enabled, set_smart_compose_enabled,
};
use crate::model::{InboxState, SetAutoAdvanceEnabled, SetOfflineEnabled, SetSmartComposeEnabled};
use fission::prelude::*;

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
                                cx.bind(
                                    SetSmartComposeEnabled(!state.smart_compose_enabled),
                                    reduce_with!(set_smart_compose_enabled),
                                ),
                            ),
                            LabToggle(
                                Icon::svg(material::action::report_problem::regular()),
                                "settings.labs.offline",
                                state.offline_enabled,
                                cx.bind(
                                    SetOfflineEnabled(!state.offline_enabled),
                                    reduce_with!(set_offline_enabled),
                                ),
                            ),
                            LabToggle(
                                Icon::svg(material::action::info::regular()),
                                "settings.labs.auto_advance",
                                state.auto_advance_enabled,
                                cx.bind(
                                    SetAutoAdvanceEnabled(!state.auto_advance_enabled),
                                    reduce_with!(set_auto_advance_enabled),
                                ),
                            ),
                        ],
                    ),
                ),
            ],
        )
}

fission_component! {
/// One experimental feature with its switch.
    LabToggle<()> {
        icon: Icon,
        label_key: &'static str,
        checked: bool,
        on_toggle: ActionEnvelope,
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
                    on_toggle: toggle.on_toggle.clone(),
                ),
            ],
        )
}
