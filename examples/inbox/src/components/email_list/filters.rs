use crate::model::app_state::SIZE_FILTER_MAX_MB;
use crate::model::list::{
    set_advanced_filters_open, set_date_filter, set_date_filter_end_open,
    set_date_filter_start_open, set_size_filter,
};
use crate::model::{
    InboxState, SetAdvancedFiltersOpen, SetDateFilter, SetDateFilterEndOpen,
    SetDateFilterStartOpen, SetSizeFilter,
};
use fission::core::op::BoxShadow;
use fission::core::ui::{Button, ButtonVariant, Container, Text, TextContent, Widget};
use fission::core::{reduce_with, Length, WidgetId};
use fission::icons::material;
use fission::widgets::{DateRangePicker, HStack, Icon, Popover, RangeSlider, VStack};
use std::sync::Arc;

/// The filters button and the date and size filters it reveals.
pub(super) struct AdvancedFilters;

impl From<AdvancedFilters> for Widget {
    fn from(_: AdvancedFilters) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        // The picker fills in the chosen range.
        let date_action = ctx.bind(SetDateFilter(None, None), reduce_with!(set_date_filter));
        let caption = |key: &str| {
            Text::new(TextContent::Key(key.into()))
                .size(tokens.typography.font_size_xs)
                .color(tokens.colors.text_secondary)
        };

        Popover {
            id: WidgetId::explicit("advanced_filters"),
            is_open: state.show_advanced_filters,
            on_close: Some(ctx.bind(
                SetAdvancedFiltersOpen(false),
                reduce_with!(set_advanced_filters_open),
            )),
            trigger: Button {
                variant: ButtonVariant::Outline,
                child: Some(
                    HStack {
                        spacing: Some(tokens.spacing.xs),
                        children: vec![
                            Icon::svg(material::content::filter_list::regular())
                                .size(tokens.typography.font_size_lg)
                                .into(),
                            Text::new(TextContent::Key("header.filters".into())).into(),
                        ],
                    }
                    .into(),
                ),
                on_press: Some(ctx.bind(
                    SetAdvancedFiltersOpen(!state.show_advanced_filters),
                    reduce_with!(set_advanced_filters_open),
                )),
                ..Default::default()
            }
            .into(),
            content: Container::new(VStack {
                spacing: Some(tokens.spacing.m),
                children: vec![
                    caption("filter.date_range").into(),
                    DateRangePicker {
                        id_start: WidgetId::explicit("filter_date_start"),
                        id_end: WidgetId::explicit("filter_date_end"),
                        start: state.date_filter.0,
                        end: state.date_filter.1,
                        is_start_open: state.date_filter_start_open,
                        is_end_open: state.date_filter_end_open,
                        on_change: Some(Arc::new(move |start, end| {
                            date_action.with_action(&SetDateFilter(start, end))
                        })),
                        on_toggle_start: Some(ctx.bind(
                            SetDateFilterStartOpen(!state.date_filter_start_open),
                            reduce_with!(set_date_filter_start_open),
                        )),
                        on_toggle_end: Some(ctx.bind(
                            SetDateFilterEndOpen(!state.date_filter_end_open),
                            reduce_with!(set_date_filter_end_open),
                        )),
                        on_close_start: Some(ctx.bind(
                            SetDateFilterStartOpen(false),
                            reduce_with!(set_date_filter_start_open),
                        )),
                        on_close_end: Some(ctx.bind(
                            SetDateFilterEndOpen(false),
                            reduce_with!(set_date_filter_end_open),
                        )),
                    }
                    .into(),
                    caption("filter.size_mb").into(),
                    RangeSlider {
                        id: Some(WidgetId::explicit("filter_size")),
                        semantics_identifier: Some("inbox.filters.size".into()),
                        start: state.size_filter_mb.0,
                        end: state.size_filter_mb.1,
                        min: 0.0,
                        max: SIZE_FILTER_MAX_MB,
                        step: Some(1.0),
                        on_change: Some(ctx.bind(SetSizeFilter, reduce_with!(set_size_filter))),
                    }
                    .into(),
                ],
            })
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
            }))
            .into(),
            motion: None,
        }
        .into()
    }
}
