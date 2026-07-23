use fission::prelude::*;

use super::lane_dropzone_panel::{LaneDropzonePanel, LanePanelState};
use super::{drag_entered, drag_left, drop_on_lane, DragEntered, DragLeft, DropOnLane};
use crate::GalleryState;

#[derive(Clone)]
pub(super) struct LaneDropzone {
    pub id: &'static str,
    pub title: &'static str,
    pub items: Vec<String>,
}

impl From<LaneDropzone> for Widget {
    fn from(lane: LaneDropzone) -> Self {
        let (ctx, _) = fission::build::current::<GalleryState>();
        let identifier = format!("gallery.drag.zone.{}", lane.id);

        Dropzone {
            id: Some(WidgetId::explicit(&identifier)),
            semantics_identifier: Some(identifier),
            child: LaneDropzonePanel {
                title: lane.title,
                items: lane.items.clone(),
                state: LanePanelState::Idle,
                instance: match lane.id {
                    "Backlog" => "backlog.idle",
                    _ => "done.idle",
                },
            }
            .into(),
            active_child: Some(
                LaneDropzonePanel {
                    title: lane.title,
                    items: lane.items.clone(),
                    state: LanePanelState::Active,
                    instance: match lane.id {
                        "Backlog" => "backlog.active",
                        _ => "done.active",
                    },
                }
                .into(),
            ),
            hover_child: Some(
                LaneDropzonePanel {
                    title: lane.title,
                    items: lane.items,
                    state: LanePanelState::Hovered,
                    instance: match lane.id {
                        "Backlog" => "backlog.hover",
                        _ => "done.hover",
                    },
                }
                .into(),
            ),
            on_drop: Some(ctx.bind(DropOnLane(lane.title.into()), reduce_with!(drop_on_lane))),
            on_drag_enter: Some(
                ctx.bind(DragEntered(lane.title.into()), reduce_with!(drag_entered)),
            ),
            on_drag_leave: Some(ctx.bind(DragLeft(lane.title.into()), reduce_with!(drag_left))),
        }
        .into()
    }
}
