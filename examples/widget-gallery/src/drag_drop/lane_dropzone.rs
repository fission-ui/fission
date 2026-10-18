use fission::prelude::*;

use super::lane_dropzone_panel::{LaneDropzonePanel, LanePanelState};
use super::{drag_entered, drag_left, drop_on_lane, DragEntered, DragLeft, DropOnLane};
use crate::GalleryState;

fission_component! {
    #[derive(Clone)] pub(super) LaneDropzone<GalleryState> {
        id: &'static str,
        title: &'static str,
        items: Vec<String>,
    }
    |lane, _state, cx| {
        let identifier = format!("gallery.drag.zone.{}", lane.id);
        // The zone shows one of its idle, active and hover panels at a time. Cards keep the same
        // identity in each, so the card being dragged is not replaced when the panel changes.

    } =>
        Dropzone(
            id: Some(WidgetId::explicit(&identifier)),
            semantics_identifier: Some(identifier),
            child: LaneDropzonePanel(
                lane.title,
                lane.items.clone(),
                LanePanelState::Idle,
                match lane.id {
                    "Backlog" => "backlog",
                    _ => "done",
                },
            ),
            active_child: LaneDropzonePanel(
                    lane.title,
                    lane.items.clone(),
                    LanePanelState::Active,
                    match lane.id {
                        "Backlog" => "backlog",
                        _ => "done",
                    },
                ),
            hover_child: LaneDropzonePanel(
                    lane.title,
                    lane.items.clone(),
                    LanePanelState::Hovered,
                    match lane.id {
                        "Backlog" => "backlog",
                        _ => "done",
                    },
                ),
            on_drop: cx.build.bind(DropOnLane(lane.title.into()), reduce_with!(drop_on_lane)),
            on_drag_enter: cx.build.bind(DragEntered(lane.title.into()), reduce_with!(drag_entered)),
            on_drag_leave: cx.build.bind(DragLeft(lane.title.into()), reduce_with!(drag_left)),
        )
}
