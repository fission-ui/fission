use crate::model::app_state::SIZE_FILTER_MAX_MB;
use crate::model::list::{
    set_advanced_filters_open, set_date_filter, set_date_filter_end_open,
    set_date_filter_start_open, set_size_filter,
};
use crate::model::{
    InboxState, SetAdvancedFiltersOpen, SetDateFilter, SetDateFilterEndOpen,
    SetDateFilterStartOpen, SetSizeFilter,
};
use fission::prelude::*;
use std::sync::Arc;

fission_component! {
/// The filters button and the date and size filters it reveals.
    pub(super) AdvancedFilters<InboxState> |_filters, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let date_action = cx.bind(SetDateFilter(None, None), reduce_with!(set_date_filter));
        let caption = |key: &str| {
            Text::new(TextContent::Key(key.into()))
                .size(tokens.typography.font_size_xs)
                .color(tokens.colors.text_secondary)
        };
    } =>
        Popover(
            id: WidgetId::explicit("advanced_filters"),
            is_open: state.show_advanced_filters,
            on_close: cx.bind(
                SetAdvancedFiltersOpen(false),
                reduce_with!(set_advanced_filters_open),
            ),
            trigger: Button(
                variant: ButtonVariant::Outline,
                child: HStack(
                        spacing: tokens.spacing.xs,
                        children: [
                            Icon::svg(material::content::filter_list::regular())
                                .size(tokens.typography.font_size_lg),
                            Text(TextContent::Key("header.filters".into())),
                        ],
                ),
                on_press: cx.bind(
                    SetAdvancedFiltersOpen(!state.show_advanced_filters),
                    reduce_with!(set_advanced_filters_open),
                ),
            ),
            content: Container(
                VStack(
                spacing: tokens.spacing.m,
                children: [
                    caption("filter.date_range"),
                    DateRangePicker(
                        id_start: WidgetId::explicit("filter_date_start"),
                        id_end: WidgetId::explicit("filter_date_end"),
                        start: state.date_filter.0,
                        end: state.date_filter.1,
                        is_start_open: state.date_filter_start_open,
                        is_end_open: state.date_filter_end_open,
                        on_change: Arc::new(move |start, end| {
                            date_action.with_action(&SetDateFilter(start, end))
                        }) as Arc<
                            dyn Fn(Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) -> ActionEnvelope
                                + Send
                                + Sync,
                        >,
                        on_toggle_start: cx.bind(
                            SetDateFilterStartOpen(!state.date_filter_start_open),
                            reduce_with!(set_date_filter_start_open),
                        ),
                        on_toggle_end: cx.bind(
                            SetDateFilterEndOpen(!state.date_filter_end_open),
                            reduce_with!(set_date_filter_end_open),
                        ),
                        on_close_start: cx.bind(
                            SetDateFilterStartOpen(false),
                            reduce_with!(set_date_filter_start_open),
                        ),
                        on_close_end: cx.bind(
                            SetDateFilterEndOpen(false),
                            reduce_with!(set_date_filter_end_open),
                        ),
                    ),
                    caption("filter.size_mb"),
                    RangeSlider(
                        id: Some(WidgetId::explicit("filter_size")),
                        semantics_identifier: Some("inbox.filters.size".into()),
                        start: state.size_filter_mb.0,
                        end: state.size_filter_mb.1,
                        min: 0.0,
                        max: SIZE_FILTER_MAX_MB,
                        step: Some(1.0),
                        on_change: cx.bind(SetSizeFilter, reduce_with!(set_size_filter)),
                    ),
                ],
                ),
            )
            .width_length(Length::clamp(
                Length::points(240.0),
                Length::percent(34.0),
                Length::points(320.0),
            ))
            .padding_all(tokens.spacing.m)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.medium)
            .shadow(tokens.elevations.level2.unwrap_or(BoxShadow {
                spread_radius: 0.0,
                inset: false,
                color: tokens.colors.text_primary.with_alpha(40),
                blur_radius: tokens.spacing.s,
                offset: (0.0, tokens.spacing.xs),
            })),
        )
}
