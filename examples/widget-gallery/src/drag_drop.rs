use fission::core::op::Color as IrColor;
use fission::core::ui::{Button, ButtonVariant, Container, Text, Widget};
use fission::core::{reduce_with, ReducerContext, WidgetId};
use fission::prelude::fission_action;
use fission::widgets::{DragPreviewOptions, DragTarget, Draggable, Dropzone, HStack, Tag, VStack};

use crate::GalleryState;

#[derive(Clone)]
pub(crate) struct DragDropSection;

#[derive(Clone)]
struct TaskCard {
    label: String,
    snap_to_grid: bool,
}

#[derive(Clone)]
struct LaneDropzone {
    id: &'static str,
    title: &'static str,
    items: Vec<String>,
}

#[derive(Clone)]
struct ExternalFileDropTarget;

#[derive(Clone)]
struct DragLog;

#[fission_action]
struct ToggleSnapPreview;

#[fission_action]
struct DragStarted(String);

#[fission_action]
struct DragEnded(String);

#[fission_action]
struct DropOnLane(String);

#[fission_action]
struct DragEntered(String);

#[fission_action]
struct DragLeft(String);

#[fission_action]
struct ExternalFilesDropped;

fn toggle_snap_preview(
    state: &mut GalleryState,
    _: ToggleSnapPreview,
    _: &mut ReducerContext<GalleryState>,
) {
    state.drag_snap_preview = !state.drag_snap_preview;
}

fn drag_started(
    state: &mut GalleryState,
    action: DragStarted,
    _: &mut ReducerContext<GalleryState>,
) {
    push_drag_log(state, format!("Started dragging {}", action.0));
}

fn drag_ended(state: &mut GalleryState, action: DragEnded, _: &mut ReducerContext<GalleryState>) {
    push_drag_log(state, format!("Ended dragging {}", action.0));
    state.drag_hover_zone = None;
}

fn drag_entered(
    state: &mut GalleryState,
    action: DragEntered,
    _: &mut ReducerContext<GalleryState>,
) {
    state.drag_hover_zone = Some(action.0.clone());
    push_drag_log(state, format!("Hovering over {}", action.0));
}

fn drag_left(state: &mut GalleryState, action: DragLeft, _: &mut ReducerContext<GalleryState>) {
    if state.drag_hover_zone.as_deref() == Some(action.0.as_str()) {
        state.drag_hover_zone = None;
    }
    push_drag_log(state, format!("Left {}", action.0));
}

fn drop_on_lane(
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

fn external_files_dropped(
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
    fn from(_: DragDropSection) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;

        VStack {
            spacing: Some(12.0),
            children: vec![
                Text::new("Drag and Drop").size(20.0).into(),
                Text::new(
                    "Drag task cards between lanes, hover over targets to see accepted-state feedback, or drop external files onto the import target.",
                )
                .color(tokens.colors.text_secondary)
                .into(),
                HStack {
                    spacing: Some(10.0),
                    children: vec![
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
                        .semantics_identifier("gallery.drag.snap_preview")
                        .into(),
                        Text::new("The preview avatar follows the pointer; snapping shows grid-constrained drag UX.")
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                }
                .into(),
                HStack {
                    spacing: Some(16.0),
                    children: vec![
                        LaneDropzone {
                            id: "Backlog",
                            title: "Backlog",
                            items: state.drag_backlog.clone(),
                        }
                        .into(),
                        LaneDropzone {
                            id: "Done",
                            title: "Done",
                            items: state.drag_done.clone(),
                        }
                        .into(),
                        VStack {
                            spacing: Some(10.0),
                            children: vec![ExternalFileDropTarget.into(), DragLog.into()],
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        }
        .into()
    }
}

impl From<TaskCard> for Widget {
    fn from(card: TaskCard) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let preview_options = DragPreviewOptions {
            snap_grid: card.snap_to_grid.then_some(24.0),
            ..Default::default()
        };

        Draggable {
            id: Some(WidgetId::explicit(&format!(
                "gallery.drag.card.{}",
                card.label
            ))),
            semantics_identifier: Some(format!("gallery.drag.card.{}", card.label)),
            payload: card.label.as_bytes().to_vec(),
            child: Container::new(HStack {
                spacing: Some(8.0),
                children: vec![
                    Text::new("::").color(tokens.colors.text_secondary).into(),
                    Text::new(card.label.clone()).into(),
                ],
            })
            .padding_all(10.0)
            .border(tokens.colors.border, 1.0)
            .border_radius(10.0)
            .bg(tokens.colors.surface)
            .into(),
            preview: Some(
                Container::new(HStack {
                    spacing: Some(6.0),
                    children: vec![
                        Text::new("move")
                            .size(11.0)
                            .color(tokens.colors.primary)
                            .into(),
                        Text::new(card.label.clone()).into(),
                    ],
                })
                .padding_all(10.0)
                .border(tokens.colors.primary, 1.0)
                .border_radius(12.0)
                .bg(tokens.colors.surface.with_alpha(235))
                .into(),
            ),
            preview_options,
            on_drag_start: Some(
                ctx.bind(DragStarted(card.label.clone()), reduce_with!(drag_started)),
            ),
            on_drag_end: Some(ctx.bind(DragEnded(card.label), reduce_with!(drag_ended))),
        }
        .into()
    }
}

impl From<LaneDropzone> for Widget {
    fn from(lane: LaneDropzone) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let title = lane.title;
        let is_hovered = state.drag_hover_zone.as_deref() == Some(title);
        let mut children: Vec<Widget> = vec![
            HStack {
                spacing: Some(8.0),
                children: vec![
                    Text::new(title).size(16.0).into(),
                    Tag {
                        label: format!("{}", lane.items.len()),
                        on_close: None,
                    }
                    .into(),
                ],
            }
            .into(),
            Text::new(if is_hovered {
                "Release to drop here"
            } else {
                "Drag cards into this lane"
            })
            .size(12.0)
            .color(tokens.colors.text_secondary)
            .into(),
        ];
        for item in lane.items {
            children.push(
                TaskCard {
                    label: item,
                    snap_to_grid: state.drag_snap_preview,
                }
                .into(),
            );
        }

        let idle = Container::new(VStack {
            spacing: Some(8.0),
            children: children.clone(),
        })
        .width(210.0)
        .min_height(190.0)
        .padding_all(12.0)
        .border(tokens.colors.border, 1.0)
        .border_radius(14.0)
        .bg(tokens.colors.background.with_alpha(35))
        .into();

        let active = Container::new(VStack {
            spacing: Some(8.0),
            children: children.clone(),
        })
        .width(210.0)
        .min_height(190.0)
        .padding_all(12.0)
        .border(tokens.colors.primary.with_alpha(150), 1.0)
        .border_radius(14.0)
        .bg(tokens.colors.primary.with_alpha(18))
        .into();

        let hover = Container::new(VStack {
            spacing: Some(8.0),
            children,
        })
        .width(210.0)
        .min_height(190.0)
        .padding_all(12.0)
        .border(tokens.colors.primary, 2.0)
        .border_radius(14.0)
        .bg(tokens.colors.primary.with_alpha(36))
        .into();

        Dropzone {
            id: Some(WidgetId::explicit(&format!(
                "gallery.drag.zone.{}",
                lane.id
            ))),
            semantics_identifier: Some(format!("gallery.drag.zone.{}", lane.id)),
            child: idle,
            active_child: Some(active),
            hover_child: Some(hover),
            on_drop: Some(ctx.bind(DropOnLane(title.into()), reduce_with!(drop_on_lane))),
            on_drag_enter: Some(ctx.bind(DragEntered(title.into()), reduce_with!(drag_entered))),
            on_drag_leave: Some(ctx.bind(DragLeft(title.into()), reduce_with!(drag_left))),
        }
        .into()
    }
}

impl From<ExternalFileDropTarget> for Widget {
    fn from(_: ExternalFileDropTarget) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let mut file_widgets = vec![Text::new("External files").size(16.0).into()];
        if state.drag_external_files.is_empty() {
            file_widgets.push(
                Text::new("Drop files from Finder, Explorer, or the test driver here.")
                    .size(12.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );
        } else {
            for file in state.drag_external_files.iter().take(4) {
                file_widgets.push(Text::new(file.clone()).size(12.0).into());
            }
        }

        DragTarget {
            id: Some(WidgetId::explicit("gallery.drag.external_files")),
            semantics_identifier: Some("gallery.drag.external_files".into()),
            on_drop: Some(ctx.bind(ExternalFilesDropped, reduce_with!(external_files_dropped))),
            child: Container::new(VStack {
                spacing: Some(6.0),
                children: file_widgets.clone(),
            })
            .width(230.0)
            .min_height(86.0)
            .padding_all(12.0)
            .border(tokens.colors.border, 1.0)
            .border_radius(14.0)
            .bg(tokens.colors.background.with_alpha(28))
            .into(),
            hover_child: Some(
                Container::new(VStack {
                    spacing: Some(6.0),
                    children: file_widgets,
                })
                .width(230.0)
                .min_height(86.0)
                .padding_all(12.0)
                .border(tokens.colors.primary, 2.0)
                .border_radius(14.0)
                .bg(tokens.colors.primary.with_alpha(32))
                .into(),
            ),
        }
        .into()
    }
}

impl From<DragLog> for Widget {
    fn from(_: DragLog) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let rows: Vec<Widget> = if state.drag_log.is_empty() {
            vec![Text::new("Drag log is empty.")
                .size(12.0)
                .color(tokens.colors.text_secondary)
                .into()]
        } else {
            state
                .drag_log
                .iter()
                .map(|entry| Text::new(entry.clone()).size(12.0).into())
                .collect()
        };

        Container::new(VStack {
            spacing: Some(5.0),
            children: vec![
                Text::new("Interaction log").size(16.0).into(),
                VStack {
                    spacing: Some(4.0),
                    children: rows,
                }
                .into(),
            ],
        })
        .width(230.0)
        .min_height(116.0)
        .padding_all(12.0)
        .border(
            IrColor {
                r: 190,
                g: 190,
                b: 190,
                a: 140,
            },
            1.0,
        )
        .border_radius(14.0)
        .into()
    }
}
