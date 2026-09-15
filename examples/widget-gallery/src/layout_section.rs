//! Tables, panes and surfaces that hold other content.

use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{
    DataTable, HStack, Popover, RefreshIndicator, RefreshIndicatorStatus, SplitDirection,
    SplitView, TableColumn, TableRow, VStack,
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

fn caption(text: impl Into<String>) -> Widget {
    let (_, view) = fission::build::current::<()>();
    Text::new(text.into())
        .color(view.env().theme.tokens.colors.text_secondary)
        .wrap(true)
        .into()
}

pub(crate) fn data_table() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let mut team: Vec<_> = TEAM.to_vec();
    team.sort_by(|a, b| a.1.cmp(b.1));
    if !state.table_sort_ascending {
        team.reverse();
    }
    let toggle_row = with_reducer!(ctx, ToggleTableRow(String::new()), toggle_table_row);
    vec![
        DataTable {
            id: WidgetId::explicit("gallery.data_table"),
            columns: vec![
                TableColumn {
                    id: "name".into(),
                    title: "Name".into(),
                    width: NAME_COLUMN_WIDTH,
                    sortable: true,
                    on_sort: Some(with_reducer!(ctx, ToggleTableSort, toggle_table_sort)),
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
                .collect(),
            selected_ids: state.table_selected.clone(),
            on_selection_change: Some(Arc::new(move |id| {
                toggle_row.with_action(&ToggleTableRow(id))
            })),
            on_select_all: Some(with_reducer!(
                ctx,
                ToggleAllTableRows,
                toggle_all_table_rows
            )),
            label: Some("Team members".into()),
        }
        .into(),
        caption(format!(
            "{} of {} selected",
            state.table_selected.len(),
            TEAM.len()
        )),
    ]
}

fn pane(title: &str, body: &str) -> Widget {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    Container::new(VStack {
        spacing: Some(tokens.spacing.xs),
        children: widgets![
            Text::new(title)
                .weight(tokens.typography.font_weight_semibold)
                .color(tokens.colors.text_primary),
            Text::new(body)
                .color(tokens.colors.text_secondary)
                .wrap(true),
        ],
    })
    .padding_all(tokens.spacing.m)
    .into()
}

pub(crate) fn split_view() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let presets: Vec<Widget> = SPLIT_RATIOS
        .iter()
        .map(|(ratio, label, slug)| {
            let active = (state.split_ratio - ratio).abs() < f32::EPSILON;
            Button {
                variant: if active {
                    ButtonVariant::SecondaryGray
                } else {
                    ButtonVariant::Outline
                },
                size: ComponentSize::Sm,
                child: Some(Text::new(*label).into()),
                on_press: Some(with_reducer!(ctx, SetSplitRatio(*ratio), set_split_ratio)),
                ..Default::default()
            }
            .semantics_identifier(format!("gallery.split_view.{slug}"))
            .into()
        })
        .collect();
    vec![
        HStack {
            spacing: Some(tokens.spacing.s),
            children: presets,
        }
        .into(),
        Container::new(SplitView {
            id: WidgetId::explicit("gallery.split_view"),
            direction: SplitDirection::Horizontal,
            first: pane("Folders", "Inbox, Drafts, Sent and Archive."),
            second: pane(
                "Message",
                "The selected message opens here, next to the list of folders.",
            ),
            split_ratio: state.split_ratio,
            on_resize: None,
        })
        .width_length(Length::percent(100.0))
        .height(SPLIT_HEIGHT)
        .border(tokens.colors.border, tokens.sizing.border_hairline)
        .border_radius(tokens.radii.medium)
        .into(),
    ]
}

pub(crate) fn popover() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let close = with_reducer!(ctx, ClosePopover, close_popover);
    let content: Widget = Container::new(VStack {
        spacing: Some(tokens.spacing.s),
        children: widgets![
            Text::new("Share this page")
                .weight(tokens.typography.font_weight_semibold)
                .color(tokens.colors.text_primary),
            Text::new("Anyone with the link can view it.")
                .color(tokens.colors.text_secondary)
                .wrap(true),
            HStack {
                spacing: None,
                children: widgets![Button {
                    variant: ButtonVariant::Filled,
                    size: ComponentSize::Sm,
                    child: Some(Text::new("Copy link").into()),
                    on_press: Some(close.clone()),
                    ..Default::default()
                }
                .semantics_identifier("gallery.popover.copy")],
            },
        ],
    })
    .width(POPOVER_WIDTH)
    .padding_all(tokens.spacing.m)
    .bg(tokens.colors.surface)
    .border(tokens.colors.border, tokens.sizing.border_hairline)
    .border_radius(tokens.radii.large)
    .into();
    widgets![HStack {
        spacing: None,
        children: widgets![Popover {
            id: WidgetId::explicit("gallery.popover"),
            is_open: state.popover_open,
            on_close: Some(close),
            trigger: Button {
                variant: ButtonVariant::Outline,
                child: Some(Text::new("Share").into()),
                on_press: Some(with_reducer!(ctx, TogglePopover, toggle_popover)),
                ..Default::default()
            }
            .semantics_identifier("gallery.popover.trigger")
            .into(),
            content,
            motion: None,
        }],
    }]
}

pub(crate) fn refresh_indicator() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let rows: Vec<Widget> = state
        .refresh_items
        .iter()
        .map(|item| {
            Container::new(Text::new(item.clone()).color(tokens.colors.text_primary))
                .width_length(Length::percent(100.0))
                .padding_all(tokens.spacing.s)
                .bg(tokens.colors.surface)
                .border_radius(tokens.radii.medium)
                .into()
        })
        .collect();
    let list: Widget = Container::new(VStack {
        spacing: Some(tokens.spacing.xs),
        children: rows,
    })
    .width_length(Length::percent(100.0))
    .height_length(Length::percent(100.0))
    .padding_all(tokens.spacing.s)
    .into();
    let mut indicator = RefreshIndicator::new(list)
        .status(state.refresh_status)
        .pulled_extent(state.refresh_extent)
        .trigger_distance(REFRESH_TRIGGER)
        .displacement(REFRESH_DISPLACEMENT)
        .on_pull_start(with_reducer!(ctx, PullStarted, pull_started))
        .on_pull_update(with_reducer!(ctx, PullUpdated, pull_updated))
        .on_pull_cancel(with_reducer!(ctx, PullCancelled, pull_cancelled))
        .on_refresh(with_reducer!(ctx, StartRefresh, start_refresh));
    indicator.id = WidgetId::explicit("gallery.refresh_indicator");

    let refreshing = state.refresh_status == RefreshIndicatorStatus::Refreshing;
    // There is no real feed behind the demo, so a button stands in for the work finishing.
    let action: Widget = Button {
        variant: ButtonVariant::Outline,
        size: ComponentSize::Sm,
        child: Some(
            Text::new(if refreshing {
                "Finish refresh"
            } else {
                "Refresh"
            })
            .into(),
        ),
        on_press: Some(if refreshing {
            with_reducer!(ctx, FinishRefresh, finish_refresh)
        } else {
            with_reducer!(ctx, StartRefresh, start_refresh)
        }),
        ..Default::default()
    }
    .semantics_identifier("gallery.refresh_indicator.action")
    .into();

    vec![
        caption("Drag the list down and let go, or use the button."),
        Container::new(indicator)
            .width_length(Length::percent(100.0))
            .height(REFRESH_HEIGHT)
            .border(tokens.colors.border, tokens.sizing.border_hairline)
            .border_radius(tokens.radii.medium)
            .into(),
        HStack {
            spacing: None,
            children: vec![action],
        }
        .into(),
    ]
}
