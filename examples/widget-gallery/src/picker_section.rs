//! Date, time and option pickers.

use crate::state::GalleryState;
use chrono::NaiveDate;
use fission::prelude::*;
use fission::widgets::{
    Calendar, Combobox, DatePicker, DateRangePicker, DropDown, HStack, Popover, TimePicker, VStack,
};
use std::sync::Arc;

const COMBOBOX_WIDTH: f32 = 260.0;
const DROPDOWN_MENU_WIDTH: f32 = 200.0;
const FRUITS: &[&str] = &[
    "Apple",
    "Apricot",
    "Banana",
    "Blueberry",
    "Cherry",
    "Grape",
    "Lemon",
    "Mango",
    "Orange",
    "Peach",
];
const REGIONS: &[&str] = &["Europe", "North America", "Asia Pacific"];

#[fission_reducer(SelectCalendarDate)]
fn select_calendar_date(state: &mut GalleryState, date: NaiveDate) {
    state.calendar_selected = Some(date);
}

#[fission_reducer(NavigateCalendar)]
fn navigate_calendar(state: &mut GalleryState, year: i32, month: u32) {
    state.calendar_view = (year, month);
}

#[fission_reducer(ToggleDatePicker)]
fn toggle_date_picker(state: &mut GalleryState) {
    state.date_picker_open = !state.date_picker_open;
}

#[fission_reducer(CloseDatePicker)]
fn close_date_picker(state: &mut GalleryState) {
    state.date_picker_open = false;
}

#[fission_reducer(PickDate)]
fn pick_date(state: &mut GalleryState, date: NaiveDate) {
    state.date_picker_value = Some(date);
    state.date_picker_view = None;
    state.date_picker_open = false;
}

#[fission_reducer(NavigateDatePicker)]
fn navigate_date_picker(state: &mut GalleryState, year: i32, month: u32) {
    state.date_picker_view = Some((year, month));
}

#[fission_reducer(SetDateRange)]
fn set_date_range(state: &mut GalleryState, start: Option<NaiveDate>, end: Option<NaiveDate>) {
    // Picking a start after the end, or an end before the start, restarts the range.
    let (start, end) = match (start, end) {
        (Some(s), Some(e)) if s > e => {
            if state.range_start_open {
                (Some(s), None)
            } else {
                (None, Some(e))
            }
        }
        range => range,
    };
    state.range_start = start;
    state.range_end = end;
    state.range_start_open = false;
    state.range_end_open = false;
}

#[fission_reducer(ToggleRangeStart)]
fn toggle_range_start(state: &mut GalleryState) {
    state.range_start_open = !state.range_start_open;
    state.range_end_open = false;
}

#[fission_reducer(ToggleRangeEnd)]
fn toggle_range_end(state: &mut GalleryState) {
    state.range_end_open = !state.range_end_open;
    state.range_start_open = false;
}

#[fission_reducer(CloseRangePickers)]
fn close_range_pickers(state: &mut GalleryState) {
    state.range_start_open = false;
    state.range_end_open = false;
}

#[fission_reducer(SetTime)]
fn set_time(state: &mut GalleryState, hour: u32, minute: u32) {
    state.time_hour = hour;
    state.time_minute = minute;
}

#[fission_reducer(UpdateComboboxQuery)]
fn update_combobox_query(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    let Some(change) = ctx.input.text_change() else {
        return;
    };
    state.combobox_value = change.new_text.clone();
    state.combobox_open = true;
}

#[fission_reducer(SelectComboboxItem)]
fn select_combobox_item(state: &mut GalleryState, item: String) {
    state.combobox_value = item;
    state.combobox_open = false;
}

#[fission_reducer(ToggleCombobox)]
fn toggle_combobox(state: &mut GalleryState) {
    state.combobox_open = !state.combobox_open;
}

#[fission_reducer(ToggleDropdown)]
fn toggle_dropdown(state: &mut GalleryState) {
    state.dropdown_open = !state.dropdown_open;
}

#[fission_reducer(CloseDropdown)]
fn close_dropdown(state: &mut GalleryState) {
    state.dropdown_open = false;
}

#[fission_reducer(SelectDropdownItem)]
fn select_dropdown_item(state: &mut GalleryState, item: String) {
    state.dropdown_value = Some(item);
    state.dropdown_open = false;
}

/// Keeps a control at its natural width inside the stretching page column.
fn hug(child: impl Into<Widget>) -> Widget {
    HStack {
        spacing: None,
        children: vec![child.into()],
    }
    .into()
}

fn caption(text: impl Into<String>) -> Widget {
    let (_, view) = fission::build::current::<()>();
    Text::new(text.into())
        .color(view.env().theme.tokens.colors.text_secondary)
        .into()
}

fn format_date(date: Option<NaiveDate>) -> String {
    date.map(|d| d.format("%-d %B %Y").to_string())
        .unwrap_or_else(|| "not set".into())
}

pub(crate) fn calendar() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let (year, month) = state.calendar_view;
    let select = with_reducer!(
        ctx,
        SelectCalendarDate(NaiveDate::default()),
        select_calendar_date
    );
    let navigate = with_reducer!(ctx, NavigateCalendar(year, month), navigate_calendar);
    vec![
        hug(Calendar {
            year,
            month,
            selected_date: state.calendar_selected,
            on_select: Some(Arc::new(move |date| {
                select.with_action(&SelectCalendarDate(date))
            })),
            on_navigate: Some(Arc::new(move |y, m| {
                navigate.with_action(&NavigateCalendar(y, m))
            })),
            cell_size: None,
            padding: None,
        }),
        caption(format!(
            "Selected: {}",
            format_date(state.calendar_selected)
        )),
    ]
}

pub(crate) fn date_picker() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let pick = with_reducer!(ctx, PickDate(NaiveDate::default()), pick_date);
    let navigate = with_reducer!(ctx, NavigateDatePicker(0, 1), navigate_date_picker);
    vec![
        hug(DatePicker {
            id: WidgetId::explicit("gallery.date_picker"),
            value: state.date_picker_value,
            is_open: state.date_picker_open,
            width: None,
            view_year: state.date_picker_view.map(|(y, _)| y),
            view_month: state.date_picker_view.map(|(_, m)| m),
            on_navigate: Some(Arc::new(move |y, m| {
                navigate.with_action(&NavigateDatePicker(y, m))
            })),
            on_change: Some(Arc::new(move |date| pick.with_action(&PickDate(date)))),
            on_toggle: Some(with_reducer!(ctx, ToggleDatePicker, toggle_date_picker)),
            on_close: Some(with_reducer!(ctx, CloseDatePicker, close_date_picker)),
        }),
        caption(format!(
            "Due date: {}",
            format_date(state.date_picker_value)
        )),
    ]
}

pub(crate) fn date_range_picker() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let change = with_reducer!(ctx, SetDateRange(None, None), set_date_range);
    let close = with_reducer!(ctx, CloseRangePickers, close_range_pickers);
    vec![
        DateRangePicker {
            id_start: WidgetId::explicit("gallery.date_range.start"),
            id_end: WidgetId::explicit("gallery.date_range.end"),
            start: state.range_start,
            end: state.range_end,
            is_start_open: state.range_start_open,
            is_end_open: state.range_end_open,
            on_change: Some(Arc::new(move |start, end| {
                change.with_action(&SetDateRange(start, end))
            })),
            on_toggle_start: Some(with_reducer!(ctx, ToggleRangeStart, toggle_range_start)),
            on_toggle_end: Some(with_reducer!(ctx, ToggleRangeEnd, toggle_range_end)),
            on_close_start: Some(close.clone()),
            on_close_end: Some(close),
        }
        .into(),
        caption(format!(
            "Trip: {} to {}",
            format_date(state.range_start),
            format_date(state.range_end)
        )),
    ]
}

pub(crate) fn time_picker() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let change = with_reducer!(ctx, SetTime(0, 0), set_time);
    vec![
        hug(TimePicker {
            hour: state.time_hour,
            minute: state.time_minute,
            on_change: Some(Arc::new(move |hour, minute| {
                change.with_action(&SetTime(hour, minute))
            })),
        }),
        caption(format!(
            "Reminder at {:02}:{:02}",
            state.time_hour, state.time_minute
        )),
    ]
}

pub(crate) fn combobox() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let query = state.combobox_value.to_lowercase();
    let items: Vec<String> = FRUITS
        .iter()
        .filter(|fruit| {
            fruit.to_lowercase().contains(&query) || FRUITS.contains(&state.combobox_value.as_str())
        })
        .map(|fruit| fruit.to_string())
        .collect();
    let select = with_reducer!(ctx, SelectComboboxItem(String::new()), select_combobox_item);
    widgets![hug(Combobox {
        id: WidgetId::explicit("gallery.combobox"),
        value: state.combobox_value.clone(),
        items,
        placeholder: None,
        is_open: state.combobox_open,
        width: Some(COMBOBOX_WIDTH),
        max_popup_height: None,
        on_input: Some(with_reducer!(
            ctx,
            UpdateComboboxQuery,
            update_combobox_query
        )),
        on_select: Some(Arc::new(move |item| {
            select.with_action(&SelectComboboxItem(item))
        })),
        on_toggle: Some(with_reducer!(ctx, ToggleCombobox, toggle_combobox)),
        semantics_identifier: Some("gallery.combobox.input".into()),
    })]
}

pub(crate) fn dropdown() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let options: Vec<Widget> = REGIONS
        .iter()
        .map(|region| {
            Button {
                variant: ButtonVariant::Ghost,
                content_align: ButtonContentAlign::Start,
                child: Some(Text::new(*region).into()),
                on_press: Some(with_reducer!(
                    ctx,
                    SelectDropdownItem(region.to_string()),
                    select_dropdown_item
                )),
                ..Default::default()
            }
            .semantics_identifier(format!(
                "gallery.dropdown.{}",
                region.to_lowercase().replace(' ', "_")
            ))
            .into()
        })
        .collect();
    let menu: Widget = Container::new(VStack {
        spacing: Some(tokens.spacing.xxs),
        children: options,
    })
    .width(DROPDOWN_MENU_WIDTH)
    .padding_all(tokens.spacing.xs)
    .bg(tokens.colors.surface)
    .border(tokens.colors.border, tokens.sizing.border_hairline)
    .border_radius(tokens.radii.medium)
    .into();
    // DropDown is only the trigger; a popover supplies the list it opens.
    widgets![hug(Popover {
        id: WidgetId::explicit("gallery.dropdown"),
        is_open: state.dropdown_open,
        on_close: Some(with_reducer!(ctx, CloseDropdown, close_dropdown)),
        trigger: DropDown {
            on_toggle: Some(with_reducer!(ctx, ToggleDropdown, toggle_dropdown)),
            selected: state.dropdown_value.clone(),
            is_open: state.dropdown_open,
            ..Default::default()
        }
        .into(),
        content: menu,
        motion: None,
    })]
}
