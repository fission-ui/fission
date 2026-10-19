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

pub(super) fn drop_on_lane(
    state: &mut GalleryState,
    lane: &str,
    ctx: &mut ReducerContext<GalleryState>,
) {
    let Some(payload) = ctx.input.as_internal_drop() else {
        return;
    };
    let label = String::from_utf8_lossy(payload).into_owned();
    remove_task(state, &label);
    match lane {
        "Backlog" => state.drag_backlog.push(label.clone()),
        "Done" => state.drag_done.push(label.clone()),
        _ => state.drag_backlog.push(label.clone()),
    }
    state.drag_hover_zone = None;
    push_drag_log(state, format!("Dropped {label} on {lane}"));
}

pub(super) fn external_files_dropped(
    state: &mut GalleryState,
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

fission_component! {
    pub(crate) DragDropSection<GalleryState> {}
    |_section, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        VStack(
            spacing: tokens.spacing.m,
            children: [
                Row(
                    gap: tokens.spacing.s,
                    wrap: ir_op::FlexWrap::Wrap,
                    children: [
                        Button(
                            variant: if state.drag_snap_preview {
                                ButtonVariant::Filled
                            } else {
                                ButtonVariant::Outline
                            },
                            child: Text(if state.drag_snap_preview {
                                "Snap preview: on"
                            } else {
                                "Snap preview: off"
                            }),
                            on_press: |_section, state, _cx| {
                                state.drag_snap_preview = !state.drag_snap_preview;
                            },
                        )
                        .semantics_identifier("gallery.drag.snap_preview"),
                        Text(
                            "The preview follows the pointer; snapping demonstrates grid-constrained drag UX.",
                        )
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_secondary),
                    ],
                ),
                Grid(
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(DROP_PANEL_MIN_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: tokens.spacing.m,
                    row_gap: tokens.spacing.m,
                    children: [
                        LaneDropzone("Backlog", "Backlog".to_string(), state.drag_backlog.clone()),
                        LaneDropzone("Done", "Done".to_string(), state.drag_done.clone()),
                        DragAuxiliaryColumn(),
                    ],
                ),
            ],
        )
}
