use super::section::SectionHeading;
use crate::model::settings::{set_inbox_type, set_locale};
use crate::model::{InboxState, SetInboxType, SetLocale};
use fission::i18n::Locale;
use fission::prelude::*;
use std::sync::Arc;

/// Locale codes and the translation key for each one's label.
const LOCALES: [(&str, &str); 2] = [("en-US", "settings.lang_en"), ("es-ES", "settings.lang_es")];

/// Inbox type values, with each one's label key and semantics identifier.
const INBOX_TYPES: [(&str, &str, &str); 2] = [
    (
        "Default",
        "settings.inbox_type.default",
        "settings.inbox-type.default",
    ),
    (
        "Priority Inbox",
        "settings.inbox_type.priority",
        "settings.inbox-type.priority",
    ),
];

fission_component! {
/// Language and inbox type.
    pub(super) GeneralSettings<InboxState> |_settings, state, cx| {
        let locale_actions: Vec<ActionEnvelope> = LOCALES
            .iter()
            .map(|(code, _)| cx.bind(SetLocale(Locale::from(*code)), reduce_with!(set_locale)))
            .collect();
        let selected_locale = LOCALES
            .iter()
            .position(|(code, _)| state.locale.0 == *code)
            .unwrap_or(0);
        let selected_inbox_type = INBOX_TYPES
            .iter()
            .find(|(value, ..)| state.inbox_type == *value)
            .unwrap_or(&INBOX_TYPES[0]);

    } =>
        VStack(
            spacing: cx.view.env().theme.tokens.spacing.m,
            children: [
                SectionHeading("settings.general"),
                SegmentedControl(
                    options: LOCALES.iter().map(|(_, key)| cx.tr(key)).collect::<Vec<_>>(),
                    selected_index: selected_locale,
                    on_change: Arc::new(move |index: usize| locale_actions[index].clone())
                        as Arc<dyn Fn(usize) -> ActionEnvelope + Send + Sync>,
                ),
                FormControl(
                    label: Some(cx.tr("settings.inbox_type.label")),
                    helper: Some(cx.tr("settings.inbox_type.helper")),
                    child: Select(
                        id: WidgetId::explicit("inbox_type_select"),
                        selected_label: Some(cx.tr(selected_inbox_type.1)),
                        placeholder: cx.tr("settings.inbox_type.placeholder"),
                        is_open: state.show_inbox_type_select,
                        on_toggle: |_settings, state, _cx| {
                            state.show_inbox_type_select = !state.show_inbox_type_select;
                            if state.show_inbox_type_select {
                                state.show_theme_select = false;
                                state.show_density_select = false;
                            }
                        },
                        items: INBOX_TYPES
                            .iter()
                            .map(|(value, key, identifier)| SelectItem {
                                label: cx.tr(key),
                                icon: None,
                                on_select: cx.bind(
                                    SetInboxType((*value).into()),
                                    reduce_with!(set_inbox_type),
                                ),
                                semantics_identifier: Some((*identifier).into()),
                                form_value: None,
                            })
                            .collect::<Vec<_>>(),
                    ),
                ),
            ],
        )
}
