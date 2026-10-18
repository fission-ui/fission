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

fission_component! {
    Hug<()> { child: Widget } |hug, _state, _cx| {} =>
    HStack(spacing: None, children: [hug.child.clone()])
}

fission_component! {
    Caption<()> { text: String } |caption, _state, cx| {} =>
    Text(caption.text.clone()).color(cx.view.env().theme.tokens.colors.text_secondary)
}

fn format_date(date: Option<NaiveDate>) -> String {
    date.map(|d| d.format("%-d %B %Y").to_string())
        .unwrap_or_else(|| "not set".into())
}

pub(crate) fn calendar() -> Vec<Widget> {
    widgets![CalendarDemo::new()]
}

fission_component! {
    CalendarDemo<GalleryState> |_demo, state, cx| {
        let (year, month) = state.calendar_view;
        let select = with_reducer!(cx.build, SelectCalendarDate(NaiveDate::default()), select_calendar_date);
        let navigate = with_reducer!(cx.build, NavigateCalendar(year, month), navigate_calendar);
    } => Column(children: [
        Hug(Calendar(
            year: year,
            month: month,
            selected_date: state.calendar_selected,
            on_select: Arc::new(move |date| {
                select.with_action(&SelectCalendarDate(date))
            }),
            on_navigate: Arc::new(move |y, m| {
                navigate.with_action(&NavigateCalendar(y, m))
            }),
            cell_size: None,
            padding: None,
        )),
        Caption(format!(
            "Selected: {}",
            format_date(state.calendar_selected)
        )),
    ])
}

pub(crate) fn date_picker() -> Vec<Widget> {
    widgets![DatePickerDemo::new()]
}

fission_component! {
    DatePickerDemo<GalleryState> |_demo, state, cx| {
        let pick = with_reducer!(cx.build, PickDate(NaiveDate::default()), pick_date);
        let navigate = with_reducer!(cx.build, NavigateDatePicker(0, 1), navigate_date_picker);
    } => Column(children: [
        Hug(DatePicker(
            id: WidgetId::explicit("gallery.date_picker"),
            value: state.date_picker_value,
            is_open: state.date_picker_open,
            width: None,
            view_year: state.date_picker_view.map(|(y, _)| y),
            view_month: state.date_picker_view.map(|(_, m)| m),
            on_navigate: Arc::new(move |y, m| {
                navigate.with_action(&NavigateDatePicker(y, m))
            }),
            on_change: Arc::new(move |date| pick.with_action(&PickDate(date))),
            on_toggle: with_reducer!(cx.build, ToggleDatePicker, toggle_date_picker),
            on_close: with_reducer!(cx.build, CloseDatePicker, close_date_picker),
        )),
        Caption(format!(
            "Due date: {}",
            format_date(state.date_picker_value)
        )),
    ])
}

pub(crate) fn date_range_picker() -> Vec<Widget> {
    widgets![DateRangeDemo::new()]
}

fission_component! {
    DateRangeDemo<GalleryState> |_demo, state, cx| {
        let change = with_reducer!(cx.build, SetDateRange(None, None), set_date_range);
        let close = with_reducer!(cx.build, CloseRangePickers, close_range_pickers);
    } => Column(children: [
        DateRangePicker(
            id_start: WidgetId::explicit("gallery.date_range.start"),
            id_end: WidgetId::explicit("gallery.date_range.end"),
            start: state.range_start,
            end: state.range_end,
            is_start_open: state.range_start_open,
            is_end_open: state.range_end_open,
            on_change: Arc::new(move |start, end| {
                change.with_action(&SetDateRange(start, end))
            }),
            on_toggle_start: with_reducer!(cx.build, ToggleRangeStart, toggle_range_start),
            on_toggle_end: with_reducer!(cx.build, ToggleRangeEnd, toggle_range_end),
            on_close_start: close.clone(),
            on_close_end: close,
        ),
        Caption(format!(
            "Trip: {} to {}",
            format_date(state.range_start),
            format_date(state.range_end)
        )),
    ])
}

pub(crate) fn time_picker() -> Vec<Widget> {
    widgets![TimePickerDemo::new()]
}

fission_component! {
    TimePickerDemo<GalleryState> |_demo, state, cx| {
        let change = with_reducer!(cx.build, SetTime(0, 0), set_time);
    } => Column(children: [
        Hug(TimePicker(
            hour: state.time_hour,
            minute: state.time_minute,
            on_change: Arc::new(move |hour, minute| {
                change.with_action(&SetTime(hour, minute))
            }),
        )),
        Caption(format!(
            "Reminder at {:02}:{:02}",
            state.time_hour, state.time_minute
        )),
    ])
}

pub(crate) fn combobox() -> Vec<Widget> {
    widgets![ComboboxDemo::new()]
}

fission_component! {
    ComboboxDemo<GalleryState> |_demo, state, cx| {
        let query = state.combobox_value.to_lowercase();
        let items: Vec<String> = FRUITS.iter()
            .filter(|fruit| fruit.to_lowercase().contains(&query) || FRUITS.contains(&state.combobox_value.as_str()))
            .map(|fruit| fruit.to_string())
            .collect();
        let select = with_reducer!(cx.build, SelectComboboxItem(String::new()), select_combobox_item);
    } => Hug(Combobox(
        id: WidgetId::explicit("gallery.combobox"),
        value: state.combobox_value.clone(),
        placeholder: Some("Search options".into()),
        items: items,
        is_open: state.combobox_open,
        width: Some(COMBOBOX_WIDTH),
        max_popup_height: None,
        on_input: with_reducer!(cx.build, UpdateComboboxQuery, update_combobox_query),
        on_select: Arc::new(move |item| select.with_action(&SelectComboboxItem(item))),
        on_toggle: with_reducer!(cx.build, ToggleCombobox, toggle_combobox),
        semantics_identifier: Some("gallery.combobox.input".into()),
    ))
}

pub(crate) fn dropdown() -> Vec<Widget> {
    widgets![DropdownDemo::new()]
}

fission_component! {
    DropdownMenu<GalleryState> |_menu, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => Container(
        VStack(spacing: tokens.spacing.xxs, children: [
            for region in REGIONS.iter() {
                Button(
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Text(*region),
                    on_press: with_reducer!(cx.build, SelectDropdownItem(region.to_string()), select_dropdown_item),
                    key: *region,
                ).semantics_identifier(format!("gallery.dropdown.{}", region.to_lowercase().replace(' ', "_")))
            }
        ])
    )
    .width(DROPDOWN_MENU_WIDTH)
    .padding_all(tokens.spacing.xs)
    .bg(tokens.colors.surface)
    .border(tokens.colors.border, tokens.sizing.border_hairline)
    .border_radius(tokens.radii.medium)
}

fission_component! {
    DropdownDemo<GalleryState> |_demo, state, cx| {} =>
    Hug(Popover(
        id: WidgetId::explicit("gallery.dropdown"),
        is_open: state.dropdown_open,
        on_close: with_reducer!(cx.build, CloseDropdown, close_dropdown),
        trigger: DropDown(
            on_toggle: with_reducer!(cx.build, ToggleDropdown, toggle_dropdown),
            selected: state.dropdown_value.clone(),
            is_open: state.dropdown_open,
        ),
        content: DropdownMenu(),
        motion: None,
    ))
}
