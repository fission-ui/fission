use crate::model::list::set_advanced_filters_open;
use crate::model::{InboxState, SetAdvancedFiltersOpen};
use fission::core::op::BoxShadow;
use fission::core::ui::{Button, ButtonVariant, Container, Text, TextContent, Widget};
use fission::core::{reduce_with, Length, WidgetId};
use fission::icons::material;
use fission::widgets::{DateRangePicker, HStack, Icon, Popover, RangeSlider, VStack};

/// The filters button and the date and size filters it reveals.
pub(super) struct AdvancedFilters;

impl From<AdvancedFilters> for Widget {
    fn from(_: AdvancedFilters) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
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
                        start: state.schedule_date,
                        end: state.schedule_date,
                        is_start_open: false,
                        is_end_open: false,
                        on_change: None,
                        on_toggle_start: None,
                        on_toggle_end: None,
                        on_close_start: None,
                        on_close_end: None,
                    }
                    .into(),
                    caption("filter.size_mb").into(),
                    RangeSlider {
                        id: None,
                        semantics_identifier: None,
                        start: 5.0,
                        end: 50.0,
                        min: 0.0,
                        max: 100.0,
                        step: None,
                        on_change: None,
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
