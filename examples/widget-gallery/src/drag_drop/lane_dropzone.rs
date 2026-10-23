use fission::prelude::*;

use super::lane_dropzone_panel::{LaneDropzonePanel, LanePanelState};
use super::{drop_on_lane, push_drag_log};
use crate::GalleryState;

fission_component! {
    #[derive(Clone)] pub(super) LaneDropzone<GalleryState> {
        id: &'static str,
        title: String,
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
                lane.title.clone(),
                lane.items.clone(),
                LanePanelState::Idle,
                match lane.id {
                    "Backlog" => "backlog",
                    _ => "done",
                },
            ),
            active_child: LaneDropzonePanel(
                    lane.title.clone(),
                    lane.items.clone(),
                    LanePanelState::Active,
                    match lane.id {
                        "Backlog" => "backlog",
                        _ => "done",
                    },
                ),
            hover_child: LaneDropzonePanel(
                    lane.title.clone(),
                    lane.items.clone(),
                    LanePanelState::Hovered,
                    match lane.id {
                        "Backlog" => "backlog",
                        _ => "done",
                    },
                ),
            on_drop: |lane, state, cx| drop_on_lane(state, &lane.title, cx),
            on_drag_enter: |lane, state, _cx| {
                state.drag_hover_zone = Some(lane.title.clone());
                push_drag_log(state, format!("Hovering over {}", lane.title));
            },
            on_drag_leave: |lane, state, _cx| {
                if state.drag_hover_zone.as_deref() == Some(lane.title.as_str()) {
                    state.drag_hover_zone = None;
                }
                push_drag_log(state, format!("Left {}", lane.title));
            },
        )
}
