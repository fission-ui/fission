use super::filters::AdvancedFilters;
use crate::model::list::{set_filter_mode, set_sort_option};
use crate::model::{InboxState, SetFilterMode, SetSortOption};
use fission::op::FlexWrap;
use fission::prelude::*;
use std::sync::Arc;

/// Filter modes with the translation key for each one's label.
const FILTER_MODES: [&str; 3] = ["filter.all", "filter.unread", "filter.starred"];

/// Sort orders the list offers, stored in state by these values.
const SORT_OPTIONS: [(&str, &str); 3] = [
    ("Newest", "sort.newest"),
    ("Oldest", "sort.oldest"),
    ("Unread", "sort.unread"),
];

fission_component! {
/// Search, filter mode, sort order and the advanced filters.
    pub(super) ListToolbar<InboxState> |_toolbar, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let filter_actions: Vec<_> = (0..FILTER_MODES.len())
            .map(|mode| cx.bind(SetFilterMode(mode), reduce_with!(set_filter_mode)))
            .collect();
        let sort_label = SORT_OPTIONS
            .iter()
            .find(|(value, _)| state.sort_option == *value)
            .map_or_else(|| state.sort_option.clone(), |(_, key)| cx.tr(key));
    } =>
        VStack(
            spacing: tokens.spacing.xs,
            children: [
                TextInput(
                    placeholder: TextContent::Key("search.placeholder".into()),
                    bind: state.search_query,
                ),
                Row(
                    gap: tokens.spacing.s,
                    wrap: FlexWrap::Wrap,
                    children: [
                        SegmentedControl(
                            options: FILTER_MODES.iter().map(|key| cx.tr(key)).collect::<Vec<_>>(),
                            selected_index: state.filter_mode,
                            on_change: Arc::new(move |mode: usize| filter_actions[mode].clone())
                                as Arc<dyn Fn(usize) -> ActionEnvelope + Send + Sync>,
                        ),
                        Spacer(flex_grow: 1.0),
                        Select(
                            id: WidgetId::explicit("inbox.sort"),
                            selected_label: Some(sort_label),
                            items: SORT_OPTIONS
                                .iter()
                                .map(|(value, key)| SelectItem {
                                    label: cx.tr(key),
                                    icon: None,
                                    on_select: cx.bind(
                                        SetSortOption((*value).into()),
                                        reduce_with!(set_sort_option),
                                    ),
                                    semantics_identifier: Some(format!(
                                        "inbox.sort.{}",
                                        value.to_lowercase()
                                    )),
                                    form_value: None,
                                })
                                .collect::<Vec<_>>(),
                            is_open: state.show_sort_menu,
                            on_toggle: |_toolbar, state, _cx| {
                                state.show_sort_menu = !state.show_sort_menu;
                            },
                            trigger_semantics_identifier: Some("inbox.sort".into()),
                            width: 140.0,
                        ),
                        AdvancedFilters(),
                    ],
                ),
            ],
        )
}
