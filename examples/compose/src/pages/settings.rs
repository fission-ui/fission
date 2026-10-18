use crate::components::Navigation;
use crate::state::AppState;
use fission::i18n::Locale;
use fission::prelude::*;

Component! {
    pub SettingsPage<AppState> |settings, state, cx| =>
    Page(
        cx.tr("settings.title"),
        navigation: Navigation(),
        content: Form(children: [
            ToggleField(
                cx.tr("settings.notifications"),
                bind: state.settings.notifications,
                semantics_identifier: "compose.settings.notifications".to_string(),
            ),
            ToggleField(
                cx.tr("settings.compact"),
                bind: state.settings.compact_rows,
                semantics_identifier: "compose.settings.compact".to_string(),
            ),
            Row(children: [
                Button(
                    child: Text(cx.tr("settings.english")),
                    key: "english",
                    variant: ButtonVariant::Outline,
                    on_press: |_, state, _| { state.locale = Locale::from("en-US"); },
                ),
                Button(
                    child: Text(cx.tr("settings.spanish")),
                    key: "spanish",
                    variant: ButtonVariant::Outline,
                    on_press: |_, state, _| { state.locale = Locale::from("es-ES"); },
                ),
            ]),
        ]),
    )
}
