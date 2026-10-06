use fission_ir::{SceneDimension, SceneTarget, WidgetId};
use fission_layout::{LayoutPoint, LayoutSnapshot};
use serde::{Deserialize, Serialize};

use crate::event::PointerKind;

/// Lifecycle phase for an action dispatched by an interactive scene object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneInteractionPhase {
    Activate,
    Start,
    Update,
    End,
    Cancel,
    LongPress,
}

/// Runtime context accompanying a normal application action from a retained
/// 2D or 3D scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneInteraction {
    pub viewport_id: WidgetId,
    pub target_id: WidgetId,
    pub scene_id: u64,
    pub node_id: Option<u64>,
    pub instance: Option<u32>,
    pub dimension: SceneDimension,
    pub phase: SceneInteractionPhase,
    pub input_kind: PointerKind,
    /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
    pub modifiers: u8,
    pub screen_point: LayoutPoint,
    pub viewport_point: LayoutPoint,
    pub screen_delta: LayoutPoint,
    pub viewport_delta: LayoutPoint,
    /// Present for 2D scenes whose semantic target carries a view-to-scene
    /// transform. For 3D, pass `viewport_point` to the public picking API.
    pub scene_point: Option<LayoutPoint>,
    pub scene_delta: Option<LayoutPoint>,
}

pub(crate) fn scene_interaction(
    target_id: WidgetId,
    target: &SceneTarget,
    phase: SceneInteractionPhase,
    point: LayoutPoint,
    delta: LayoutPoint,
    layout: &LayoutSnapshot,
    input_kind: PointerKind,
    modifiers: u8,
) -> SceneInteraction {
    let viewport_id = WidgetId::from_u128(target.viewport_id);
    let viewport_rect = layout
        .get_node_rect(viewport_id)
        .unwrap_or_else(|| fission_layout::LayoutRect::new(0.0, 0.0, 0.0, 0.0));
    let viewport_point = LayoutPoint::new(
        point.x - viewport_rect.x() + target.viewport_origin[0],
        point.y - viewport_rect.y() + target.viewport_origin[1],
    );
    let (scene_point, scene_delta) = target.view_to_scene.map_or((None, None), |matrix| {
        let transform_point = |value: LayoutPoint| {
            LayoutPoint::new(
                matrix[0] * value.x + matrix[2] * value.y + matrix[4],
                matrix[1] * value.x + matrix[3] * value.y + matrix[5],
            )
        };
        let transformed = transform_point(viewport_point);
        let transformed_delta = LayoutPoint::new(
            matrix[0] * delta.x + matrix[2] * delta.y,
            matrix[1] * delta.x + matrix[3] * delta.y,
        );
        (Some(transformed), Some(transformed_delta))
    });
    SceneInteraction {
        viewport_id,
        target_id,
        scene_id: target.scene_id,
        node_id: target.node_id,
        instance: target.instance,
        dimension: target.dimension,
        phase,
        input_kind,
        modifiers,
        screen_point: point,
        viewport_point,
        screen_delta: delta,
        viewport_delta: delta,
        scene_point,
        scene_delta,
    }
}
