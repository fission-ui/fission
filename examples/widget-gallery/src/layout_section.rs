//! Tables, panes and surfaces that hold other content.

use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{
    DataTable, DataTableSelectionMode, HStack, Popover, RefreshIndicator, RefreshIndicatorStatus,
    SplitDirection, SplitView, TableColumn, TableRow, VStack,
};
use std::sync::Arc;

const NAME_COLUMN_WIDTH: f32 = 180.0;
const ROLE_COLUMN_WIDTH: f32 = 160.0;
const STATUS_COLUMN_WIDTH: f32 = 120.0;
const SPLIT_HEIGHT: f32 = 220.0;
const SPLIT_RATIOS: &[(f32, &str, &str)] = &[
    (0.25, "25 / 75", "quarter"),
    (0.5, "50 / 50", "half"),
    (0.75, "75 / 25", "three_quarters"),
];
const POPOVER_WIDTH: f32 = 260.0;
const REFRESH_HEIGHT: f32 = 240.0;
const REFRESH_TRIGGER: f32 = 80.0;
const REFRESH_DISPLACEMENT: f32 = 48.0;
const TEAM: &[(&str, &str, &str, &str)] = &[
    ("ada", "Ada Lovelace", "Engineering", "Active"),
    ("grace", "Grace Hopper", "Platform", "Active"),
    ("alan", "Alan Turing", "Research", "Away"),
    ("katherine", "Katherine Johnson", "Data", "Active"),
    ("linus", "Linus Torvalds", "Infrastructure", "Invited"),
];

#[fission_reducer(ToggleTableRow)]
fn toggle_table_row(state: &mut GalleryState, id: String) {
    if let Some(index) = state.table_selected.iter().position(|row| *row == id) {
        state.table_selected.remove(index);
    } else {
        state.table_selected.push(id);
    }
}

#[fission_reducer(ToggleAllTableRows)]
fn toggle_all_table_rows(state: &mut GalleryState) {
    if state.table_selected.len() == TEAM.len() {
        state.table_selected.clear();
    } else {
        state.table_selected = TEAM.iter().map(|(id, ..)| id.to_string()).collect();
    }
}

#[fission_reducer(ToggleTableSort)]
fn toggle_table_sort(state: &mut GalleryState) {
    state.table_sort_ascending = !state.table_sort_ascending;
}

#[fission_reducer(SetSplitRatio, no_eq)]
fn set_split_ratio(state: &mut GalleryState, ratio: f32) {
    state.split_ratio = ratio;
}

#[fission_reducer(TogglePopover)]
fn toggle_popover(state: &mut GalleryState) {
    state.popover_open = !state.popover_open;
}

#[fission_reducer(ClosePopover)]
fn close_popover(state: &mut GalleryState) {
    state.popover_open = false;
}

#[fission_reducer(PullStarted)]
fn pull_started(state: &mut GalleryState) {
    if state.refresh_status != RefreshIndicatorStatus::Refreshing {
        state.refresh_status = RefreshIndicatorStatus::Drag;
        state.refresh_extent = 0.0;
    }
}

#[fission_reducer(PullUpdated)]
fn pull_updated(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    if state.refresh_status == RefreshIndicatorStatus::Refreshing {
        return;
    }
    let Some((_, _, _, delta_y)) = ctx.input.as_pointer() else {
        return;
    };
    state.refresh_extent = (state.refresh_extent + delta_y).max(0.0);
    state.refresh_status = if state.refresh_extent >= REFRESH_TRIGGER {
        RefreshIndicatorStatus::Armed
    } else if state.refresh_extent > 0.0 {
        RefreshIndicatorStatus::Drag
    } else {
        RefreshIndicatorStatus::Inactive
    };
}

#[fission_reducer(PullCancelled)]
fn pull_cancelled(state: &mut GalleryState) {
    if state.refresh_status != RefreshIndicatorStatus::Refreshing {
        state.refresh_status = RefreshIndicatorStatus::Inactive;
        state.refresh_extent = 0.0;
    }
}

#[fission_reducer(StartRefresh)]
fn start_refresh(state: &mut GalleryState) {
    state.refresh_status = RefreshIndicatorStatus::Refreshing;
    state.refresh_extent = REFRESH_TRIGGER;
}

#[fission_reducer(FinishRefresh)]
fn finish_refresh(state: &mut GalleryState) {
    let count = state.refresh_items.len() + 1;
    state
        .refresh_items
        .insert(0, format!("New activity #{count}"));
    state.refresh_status = RefreshIndicatorStatus::Inactive;
    state.refresh_extent = 0.0;
}

fission_component! {
    Caption<()> { text: String } |caption, _state, cx| {} =>
    Text(caption.text.clone())
        .color(cx.view.env().theme.tokens.colors.text_secondary)
        .wrap(true)
}

pub(crate) fn data_table() -> Vec<Widget> {
    widgets![DataTableDemo::new()]
}

fission_component! {
    DataTableDemo<GalleryState> |_demo, state, cx| {
        let mut team: Vec<_> = TEAM.to_vec();
        team.sort_by(|a, b| a.1.cmp(b.1));
        if !state.table_sort_ascending { team.reverse(); }
        let toggle_row = with_reducer!(cx.build, ToggleTableRow(String::new()), toggle_table_row);
    } => Column(children: [
        DataTable(
            id: WidgetId::explicit("gallery.data_table"),
            selection_mode: DataTableSelectionMode::Multiple,
            columns: vec![
                TableColumn {
                    id: "name".into(),
                    title: "Name".into(),
                    width: NAME_COLUMN_WIDTH,
                    sortable: true,
                    on_sort: Some(with_reducer!(cx.build, ToggleTableSort, toggle_table_sort)),
                    sorted_ascending: Some(state.table_sort_ascending),
                },
                TableColumn {
                    id: "team".into(),
                    title: "Team".into(),
                    width: ROLE_COLUMN_WIDTH,
                    ..Default::default()
                },
                TableColumn {
                    id: "status".into(),
                    title: "Status".into(),
                    width: STATUS_COLUMN_WIDTH,
                    ..Default::default()
                },
            ],
            rows: team
                .iter()
                .map(|(id, name, group, status)| TableRow {
                    id: id.to_string(),
                    cells: vec![name.to_string(), group.to_string(), status.to_string()],
                })
                .collect::<Vec<_>>(),
            selected_ids: state.table_selected.clone(),
            on_selection_change: Arc::new(move |id| {
                toggle_row.with_action(&ToggleTableRow(id))
            }),
            on_select_all: with_reducer!(
                cx.build,
                ToggleAllTableRows,
                toggle_all_table_rows
            ),
            label: Some("Team members".into()),
        ),
        Caption(format!(
            "{} of {} selected",
            state.table_selected.len(),
            TEAM.len()
        )),
    ])
}

fission_component! {
    Pane<()> { title: String, body: String } |pane, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => Container(
        VStack(spacing: tokens.spacing.xs, children: [
            Text(pane.title.clone())
                .weight(tokens.typography.font_weight_semibold)
                .color(tokens.colors.text_primary),
            Text(pane.body.clone())
                .color(tokens.colors.text_secondary)
                .wrap(true),
        ])
    ).padding_all(tokens.spacing.m)
}

pub(crate) fn split_view() -> Vec<Widget> {
    widgets![SplitViewDemo::new()]
}

fission_component! {
    SplitViewDemo<GalleryState> |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => Column(children: [
        HStack(spacing: tokens.spacing.s, children: [
            for (ratio, label, slug) in SPLIT_RATIOS.iter() {
                Button(
                    variant: if (state.split_ratio - ratio).abs() < f32::EPSILON {
                        ButtonVariant::SecondaryGray
                    } else {
                        ButtonVariant::Outline
                    },
                    size: ComponentSize::Sm,
                    child: Text(*label),
                    on_press: with_reducer!(cx.build, SetSplitRatio(*ratio), set_split_ratio),
                    key: *slug,
                ).semantics_identifier(format!("gallery.split_view.{slug}"))
            }
        ]),
        Container(SplitView(
            id: WidgetId::explicit("gallery.split_view"),
            direction: SplitDirection::Horizontal,
            first: Pane("Folders".to_string(), "Inbox, Drafts, Sent and Archive.".to_string()),
            second: Pane("Message".to_string(), "The selected message opens here, next to the list of folders.".to_string()),
            split_ratio: state.split_ratio,
        ))
        .width_length(Length::percent(100.0))
        .height(SPLIT_HEIGHT)
        .border(tokens.colors.border, tokens.sizing.border_hairline)
        .border_radius(tokens.radii.medium)
    ])
}

pub(crate) fn popover() -> Vec<Widget> {
    widgets![PopoverDemo::new()]
}

fission_component! {
    PopoverContent<GalleryState> |_content, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => Container(
        VStack(spacing: tokens.spacing.s, children: [
            Text("Share this page")
                .weight(tokens.typography.font_weight_semibold)
                .color(tokens.colors.text_primary),
            Text("Anyone with the link can view it.")
                .color(tokens.colors.text_secondary)
                .wrap(true),
            HStack(spacing: None, children: [Button(
                    variant: ButtonVariant::Filled,
                    size: ComponentSize::Sm,
                    child: Text("Copy link"),
                    on_press: with_reducer!(cx.build, ClosePopover, close_popover),
                ).semantics_identifier("gallery.popover.copy")]),
        ])
    ).width(POPOVER_WIDTH)
    .padding_all(tokens.spacing.m)
    .bg(tokens.colors.surface)
    .border(tokens.colors.border, tokens.sizing.border_hairline)
    .border_radius(tokens.radii.large)
}

fission_component! {
    PopoverDemo<GalleryState> |_demo, state, cx| {} =>
    HStack(spacing: None, children: [Popover(
            id: WidgetId::explicit("gallery.popover"),
            is_open: state.popover_open,
            on_close: with_reducer!(cx.build, ClosePopover, close_popover),
            trigger: Button(
                variant: ButtonVariant::Outline,
                child: Text("Share"),
                on_press: with_reducer!(cx.build, TogglePopover, toggle_popover),
            ).semantics_identifier("gallery.popover.trigger"),
            content: PopoverContent(),
            motion: None,
        )])
}

pub(crate) fn refresh_indicator() -> Vec<Widget> {
    widgets![RefreshDemo::new()]
}

fission_component! {
    RefreshList<()> { items: Vec<String> } |list, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => Container(
        VStack(spacing: tokens.spacing.xs, children: [
            for (index, item) in list.items.iter().enumerate() {
                Container(Text(item.clone()).color(tokens.colors.text_primary), key: index)
                    .width_length(Length::percent(100.0))
                    .padding_all(tokens.spacing.s)
                    .bg(tokens.colors.surface)
                    .border_radius(tokens.radii.medium)
            }
        ])
    )
    .width_length(Length::percent(100.0))
    .height_length(Length::percent(100.0))
    .padding_all(tokens.spacing.s)
}

fission_component! {
    RefreshDemo<GalleryState> |_demo, state, cx| {
    let tokens = &cx.view.env().theme.tokens;
    let mut indicator = RefreshIndicator::new(RefreshList::new(state.refresh_items.clone()))
        .status(state.refresh_status)
        .pulled_extent(state.refresh_extent)
        .trigger_distance(REFRESH_TRIGGER)
        .displacement(REFRESH_DISPLACEMENT)
        .on_pull_start(with_reducer!(cx.build, PullStarted, pull_started))
        .on_pull_update(with_reducer!(cx.build, PullUpdated, pull_updated))
        .on_pull_cancel(with_reducer!(cx.build, PullCancelled, pull_cancelled))
        .on_refresh(with_reducer!(cx.build, StartRefresh, start_refresh));
    indicator.id = WidgetId::explicit("gallery.refresh_indicator");

    let refreshing = state.refresh_status == RefreshIndicatorStatus::Refreshing;
    } => Column(children: [
        Caption("Drag the list down and let go, or use the button.".to_string()),
        Container(indicator)
            .width_length(Length::percent(100.0))
            .height(REFRESH_HEIGHT)
            .border(tokens.colors.border, tokens.sizing.border_hairline)
            .border_radius(tokens.radii.medium),
        HStack(spacing: None, children: [
            Button(
                variant: ButtonVariant::Outline,
                size: ComponentSize::Sm,
                child: Text(if refreshing { "Finish refresh" } else { "Refresh" }),
                on_press: if refreshing {
                    with_reducer!(cx.build, FinishRefresh, finish_refresh)
                } else {
                    with_reducer!(cx.build, StartRefresh, start_refresh)
                },
            ).semantics_identifier("gallery.refresh_indicator.action"),
        ]),
    ])
}
