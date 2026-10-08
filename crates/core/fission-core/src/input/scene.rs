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
    pub presentation_id: u64,
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
    let scale_x = axis_scale(target.viewport_size[0], viewport_rect.width());
    let scale_y = axis_scale(target.viewport_size[1], viewport_rect.height());
    let viewport_point = LayoutPoint::new(
        (point.x - viewport_rect.x()) * scale_x + target.viewport_origin[0],
        (point.y - viewport_rect.y()) * scale_y + target.viewport_origin[1],
    );
    let viewport_delta = LayoutPoint::new(delta.x * scale_x, delta.y * scale_y);
    let (scene_point, scene_delta) = target.view_to_scene.map_or((None, None), |matrix| {
        let transform_point = |value: LayoutPoint| {
            LayoutPoint::new(
                matrix[0] * value.x + matrix[2] * value.y + matrix[4],
                matrix[1] * value.x + matrix[3] * value.y + matrix[5],
            )
        };
        let transformed = transform_point(viewport_point);
        let transformed_delta = LayoutPoint::new(
            matrix[0] * viewport_delta.x + matrix[2] * viewport_delta.y,
            matrix[1] * viewport_delta.x + matrix[3] * viewport_delta.y,
        );
        (Some(transformed), Some(transformed_delta))
    });
    SceneInteraction {
        viewport_id,
        target_id,
        scene_id: target.scene_id,
        presentation_id: target.presentation_id,
        node_id: target.node_id,
        instance: target.instance,
        dimension: target.dimension,
        phase,
        input_kind,
        modifiers,
        screen_point: point,
        viewport_point,
        screen_delta: delta,
        viewport_delta,
        scene_point,
        scene_delta,
    }
}

fn axis_scale(scene_extent: f32, layout_extent: f32) -> f32 {
    if scene_extent.is_finite()
        && scene_extent > 0.0
        && layout_extent.is_finite()
        && layout_extent > 0.0
    {
        scene_extent / layout_extent
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use fission_ir::SceneDimension;
    use fission_layout::{LayoutNodeGeometry, LayoutRect, LayoutSize};

    use super::*;

    #[test]
    fn resized_layout_maps_points_and_deltas_to_the_declared_viewport() {
        let viewport_id = WidgetId::explicit("resized-scene");
        let target_id = WidgetId::explicit("scene-target");
        let mut layout = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
        layout.nodes.insert(
            viewport_id,
            LayoutNodeGeometry {
                rect: LayoutRect::new(10.0, 20.0, 480.0, 270.0),
                content_size: LayoutSize::new(480.0, 270.0),
            },
        );
        let target = SceneTarget {
            viewport_id: viewport_id.as_u128(),
            scene_id: 7,
            presentation_id: 9,
            node_id: Some(8),
            instance: None,
            dimension: SceneDimension::Two,
            viewport_origin: [100.0, 50.0],
            viewport_size: [960.0, 540.0],
            view_to_scene: Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
        };

        let interaction = scene_interaction(
            target_id,
            &target,
            SceneInteractionPhase::Update,
            LayoutPoint::new(250.0, 155.0),
            LayoutPoint::new(5.0, 3.0),
            &layout,
            PointerKind::Touch,
            0,
        );

        assert_eq!(interaction.viewport_point, LayoutPoint::new(580.0, 320.0));
        assert_eq!(interaction.viewport_delta, LayoutPoint::new(10.0, 6.0));
        assert_eq!(
            interaction.scene_point,
            Some(LayoutPoint::new(580.0, 320.0))
        );
        assert_eq!(interaction.scene_delta, Some(LayoutPoint::new(10.0, 6.0)));
    }
}
