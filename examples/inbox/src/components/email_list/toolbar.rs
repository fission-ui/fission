use super::filters::AdvancedFilters;
use crate::model::list::{set_filter_mode, set_sort_option, update_search};
use crate::model::{InboxState, SetFilterMode, SetSortOption, UpdateSearch};
use fission::core::reduce_with;
use fission::core::ui::widgets::Spacer;
use fission::core::ui::{TextContent, Widget};
use fission::widgets::{DropDown, HStack, SegmentedControl, TextInput, VStack};
use std::sync::Arc;

/// Filter modes with the translation key for each one's label.
const FILTER_MODES: [&str; 3] = ["filter.all", "filter.unread", "filter.starred"];

/// Sort orders the list offers, stored in state by these values.
const SORT_OPTIONS: [(&str, &str); 3] = [
    ("Newest", "sort.newest"),
    ("Oldest", "sort.oldest"),
    ("Unread", "sort.unread"),
];

/// Search, filter mode, sort order and the advanced filters.
pub(super) struct ListToolbar;

impl From<ListToolbar> for Widget {
    fn from(_: ListToolbar) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let filter_actions: Vec<_> = (0..FILTER_MODES.len())
            .map(|mode| ctx.bind(SetFilterMode(mode), reduce_with!(set_filter_mode)))
            .collect();
        let next_sort = if state.sort_option == "Newest" {
            "Oldest"
        } else {
            "Newest"
        };
        let sort_label = SORT_OPTIONS
            .iter()
            .find(|(value, _)| state.sort_option == *value)
            .map_or_else(|| state.sort_option.clone(), |(_, key)| view.tr(key));

        VStack {
            spacing: Some(tokens.spacing.xs),
            children: vec![
                TextInput {
                    value: state.search_query.clone(),
                    placeholder: Some(TextContent::Key("search.placeholder".into())),
                    on_input: Some(
                        ctx.bind(UpdateSearch(String::new()), reduce_with!(update_search)),
                    ),
                    ..Default::default()
                }
                .into(),
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: vec![
                        SegmentedControl {
                            options: FILTER_MODES.iter().map(|key| view.tr(key)).collect(),
                            selected_index: state.filter_mode,
                            on_change: Some(Arc::new(move |mode| filter_actions[mode].clone())),
                        }
                        .into(),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into(),
                        DropDown {
                            selected: Some(sort_label),
                            options: SORT_OPTIONS.iter().map(|(_, key)| view.tr(key)).collect(),
                            on_toggle: Some(ctx.bind(
                                SetSortOption(next_sort.into()),
                                reduce_with!(set_sort_option),
                            )),
                            ..Default::default()
                        }
                        .into(),
                        AdvancedFilters.into(),
                    ],
                }
                .into(),
            ],
        }
        .into()
    }
}
