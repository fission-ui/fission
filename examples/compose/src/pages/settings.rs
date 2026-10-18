use crate::components::Navigation;
use crate::state::AppState;
use fission::i18n::Locale;
use fission::prelude::*;

fission_component! {
    pub SettingsPage<AppState> |settings, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Column(
        gap: tokens.spacing.ml,
        children: [
            Text(cx.tr("settings.title")).size(tokens.typography.heading1_size),
            Navigation(),
            Row(gap: tokens.spacing.s, children: [
                Text(cx.tr("settings.notifications")),
                Switch(
                    bind: state.settings.notifications,
                    semantics_identifier: "compose.settings.notifications".to_string(),
                ),
            ]),
            Row(gap: tokens.spacing.s, children: [
                Text(cx.tr("settings.compact")),
                Switch(
                    bind: state.settings.compact_rows,
                    semantics_identifier: "compose.settings.compact".to_string(),
                ),
            ]),
            Row(gap: tokens.spacing.s, children: [
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
