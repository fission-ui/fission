use super::section::SectionHeading;
use crate::model::settings::{set_inbox_type, set_inbox_type_select_open, set_locale};
use crate::model::{InboxState, SetInboxType, SetInboxTypeSelectOpen, SetLocale};
use fission::core::ui::Widget;
use fission::core::{reduce_with, WidgetId};
use fission::i18n::Locale;
use fission::widgets::{FormControl, SegmentedControl, Select, SelectItem, VStack};
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

/// Language and inbox type.
pub(super) struct GeneralSettings;

impl From<GeneralSettings> for Widget {
    fn from(_: GeneralSettings) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let locale_actions: Vec<_> = LOCALES
            .iter()
            .map(|(code, _)| ctx.bind(SetLocale(Locale::from(*code)), reduce_with!(set_locale)))
            .collect();
        let selected_locale = LOCALES
            .iter()
            .position(|(code, _)| state.locale.0 == *code)
            .unwrap_or(0);
        let selected_inbox_type = INBOX_TYPES
            .iter()
            .find(|(value, ..)| state.inbox_type == *value)
            .unwrap_or(&INBOX_TYPES[0]);

        VStack {
            spacing: Some(view.env().theme.tokens.spacing.m),
            children: vec![
                SectionHeading {
                    key: "settings.general",
                }
                .into(),
                SegmentedControl {
                    options: LOCALES.iter().map(|(_, key)| view.tr(key)).collect(),
                    selected_index: selected_locale,
                    on_change: Some(Arc::new(move |index| locale_actions[index].clone())),
                }
                .into(),
                FormControl {
                    id: None,
                    label: Some(view.tr("settings.inbox_type.label")),
                    required: false,
                    error: None,
                    helper: Some(view.tr("settings.inbox_type.helper")),
                    child: Select {
                        id: WidgetId::explicit("inbox_type_select"),
                        selected_label: Some(view.tr(selected_inbox_type.1)),
                        placeholder: view.tr("settings.inbox_type.placeholder"),
                        is_open: state.show_inbox_type_select,
                        on_toggle: Some(ctx.bind(
                            SetInboxTypeSelectOpen(!state.show_inbox_type_select),
                            reduce_with!(set_inbox_type_select_open),
                        )),
                        items: INBOX_TYPES
                            .iter()
                            .map(|(value, key, identifier)| SelectItem {
                                label: view.tr(key),
                                icon: None,
                                on_select: ctx.bind(
                                    SetInboxType((*value).into()),
                                    reduce_with!(set_inbox_type),
                                ),
                                semantics_identifier: Some((*identifier).into()),
                            })
                            .collect(),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
            ],
        }
        .into()
    }
}
