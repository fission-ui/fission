use fission::prelude::*;

use crate::GalleryState;

mod drag_auxiliary_column;
mod drag_log;
mod external_file_drop_panel;
mod external_file_drop_target;
mod lane_dropzone;
mod lane_dropzone_panel;
mod task_card;

use drag_auxiliary_column::DragAuxiliaryColumn;
use lane_dropzone::LaneDropzone;

pub(super) const DRAG_GRID_SIZE: f32 = 24.0;
pub(super) const DROP_PANEL_MIN_WIDTH: f32 = 190.0;

#[derive(Clone)]
pub(crate) struct DragDropSection;

#[fission_action]
pub(super) struct ToggleSnapPreview;

#[fission_action]
pub(super) struct DragStarted(pub String);

#[fission_action]
pub(super) struct DragEnded(pub String);

#[fission_action]
pub(super) struct DropOnLane(pub String);

#[fission_action]
pub(super) struct DragEntered(pub String);

#[fission_action]
pub(super) struct DragLeft(pub String);

#[fission_action]
pub(super) struct ExternalFilesDropped;

pub(super) fn toggle_snap_preview(
    state: &mut GalleryState,
    _: ToggleSnapPreview,
    _: &mut ReducerContext<GalleryState>,
) {
    state.drag_snap_preview = !state.drag_snap_preview;
}

pub(super) fn drag_started(
    state: &mut GalleryState,
    action: DragStarted,
    _: &mut ReducerContext<GalleryState>,
) {
    push_drag_log(state, format!("Started dragging {}", action.0));
}

pub(super) fn drag_ended(
    state: &mut GalleryState,
    action: DragEnded,
    _: &mut ReducerContext<GalleryState>,
) {
    push_drag_log(state, format!("Ended dragging {}", action.0));
    state.drag_hover_zone = None;
}

pub(super) fn drag_entered(
    state: &mut GalleryState,
    action: DragEntered,
    _: &mut ReducerContext<GalleryState>,
) {
    state.drag_hover_zone = Some(action.0.clone());
    push_drag_log(state, format!("Hovering over {}", action.0));
}

pub(super) fn drag_left(
    state: &mut GalleryState,
    action: DragLeft,
    _: &mut ReducerContext<GalleryState>,
) {
    if state.drag_hover_zone.as_deref() == Some(action.0.as_str()) {
        state.drag_hover_zone = None;
    }
    push_drag_log(state, format!("Left {}", action.0));
}

pub(super) fn drop_on_lane(
    state: &mut GalleryState,
    action: DropOnLane,
    ctx: &mut ReducerContext<GalleryState>,
) {
    let Some(payload) = ctx.input.as_internal_drop() else {
        return;
    };
    let label = String::from_utf8_lossy(payload).into_owned();
    remove_task(state, &label);
    match action.0.as_str() {
        "Backlog" => state.drag_backlog.push(label.clone()),
        "Done" => state.drag_done.push(label.clone()),
        _ => state.drag_backlog.push(label.clone()),
    }
    state.drag_hover_zone = None;
    push_drag_log(state, format!("Dropped {label} on {}", action.0));
}

pub(super) fn external_files_dropped(
    state: &mut GalleryState,
    _: ExternalFilesDropped,
    ctx: &mut ReducerContext<GalleryState>,
) {
    let Some(paths) = ctx.input.as_drop_paths() else {
        return;
    };
    for path in paths {
        state.drag_external_files.push(path.clone());
    }
    push_drag_log(state, format!("Accepted {} external file(s)", paths.len()));
}

fn remove_task(state: &mut GalleryState, label: &str) {
    state.drag_backlog.retain(|item| item != label);
    state.drag_done.retain(|item| item != label);
}

fn push_drag_log(state: &mut GalleryState, message: String) {
    if state.drag_log.first() == Some(&message) {
        return;
    }
    state.drag_log.insert(0, message);
    state.drag_log.truncate(6);
}

impl From<DragDropSection> for Widget {
    fn from(_section: DragDropSection) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;

        VStack {
            spacing: Some(tokens.spacing.m),
            children: widgets![
                Text::new("Drag and Drop")
                    .size(tokens.typography.heading2_size)
                    .color(tokens.colors.text_primary),
                Text::new(
                    "Drag task cards between lanes, inspect accepted-state feedback, or drop external files onto the import target.",
                )
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary),
                Row {
                    gap: Some(tokens.spacing.s),
                    wrap: ir_op::FlexWrap::Wrap,
                    children: widgets![
                        Button {
                            variant: if state.drag_snap_preview {
                                ButtonVariant::Filled
                            } else {
                                ButtonVariant::Outline
                            },
                            child: Some(Text::new(if state.drag_snap_preview {
                                "Snap preview: on"
                            } else {
                                "Snap preview: off"
                            })
                            .into()),
                            on_press: Some(ctx.bind(
                                ToggleSnapPreview,
                                reduce_with!(toggle_snap_preview),
                            )),
                            ..Default::default()
                        }
                        .semantics_identifier("gallery.drag.snap_preview"),
                        Text::new(
                            "The preview follows the pointer; snapping demonstrates grid-constrained drag UX.",
                        )
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_secondary),
                    ],
                    ..Default::default()
                },
                Grid {
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(DROP_PANEL_MIN_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: Some(tokens.spacing.m),
                    row_gap: Some(tokens.spacing.m),
                    children: widgets![
                        LaneDropzone {
                            id: "Backlog",
                            title: "Backlog",
                            items: state.drag_backlog.clone(),
                        },
                        LaneDropzone {
                            id: "Done",
                            title: "Done",
                            items: state.drag_done.clone(),
                        },
                        DragAuxiliaryColumn,
                    ],
                    ..Default::default()
                },
            ],
        }
        .into()
    }
}
