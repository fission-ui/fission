use crate::components::Navigation;
use crate::state::AppState;
use fission::i18n::Locale;
use fission::prelude::*;

fission_component! {
    pub SettingsPage<AppState> |settings, state, cx| =>
    Column(
        gap: 20.0,
        children: [
            Text::new(cx.tr("settings.title")).size(28.0),
            Navigation(),
            Row(children: [
                Text(cx.tr("settings.notifications")),
                Switch(
                    bind: state.settings.notifications,
                    semantics_identifier: "compose.settings.notifications".to_string(),
                ),
            ]),
            Row(children: [
                Text(cx.tr("settings.compact")),
                Switch(
                    bind: state.settings.compact_rows,
                    semantics_identifier: "compose.settings.compact".to_string(),
                ),
            ]),
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
        ],
    )
}
