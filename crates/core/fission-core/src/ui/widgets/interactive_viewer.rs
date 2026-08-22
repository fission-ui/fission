use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use crate::ActionEnvelope;
use fission_ir::{semantics::ActionTrigger, ActionEntry, LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

pub use fission_ir::op::{
    ViewportBoundary, ViewportClip, ViewportMargin, ViewportPanAxis, ViewportTransform,
    ViewportZoomPolicy,
};

pub const DEFAULT_MIN_VIEWPORT_SCALE: f32 = 0.8;
pub const DEFAULT_MAX_VIEWPORT_SCALE: f32 = 2.5;
pub const DEFAULT_VIEWPORT_FRICTION: f32 = 0.000_013_5;

/// A retained, backend-neutral viewport that can pan and uniformly scale one child.
///
/// The runtime owns transient gesture and inertial state. `transform` is an
/// optional controlled value supplied by the application; `initial_transform`
/// seeds runtime state when no controlled value is present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveViewer {
    pub id: Option<WidgetId>,
    pub child: Widget,
    pub initial_transform: ViewportTransform,
    pub transform: Option<ViewportTransform>,
    pub pan_axis: ViewportPanAxis,
    pub boundary: ViewportBoundary,
    pub clip: ViewportClip,
    pub zoom_policy: ViewportZoomPolicy,
    pub min_scale: f32,
    pub max_scale: f32,
    pub friction: f32,
    pub on_interaction_start: Option<ActionEnvelope>,
    pub on_interaction_update: Option<ActionEnvelope>,
    pub on_interaction_end: Option<ActionEnvelope>,
}

impl InteractiveViewer {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            ..Self::default()
        }
    }
}

impl Default for InteractiveViewer {
    fn default() -> Self {
        Self {
            id: None,
            child: crate::ui::Spacer::default().into(),
            initial_transform: ViewportTransform::IDENTITY,
            transform: None,
            pan_axis: ViewportPanAxis::Both,
            boundary: ViewportBoundary::Unbounded,
            clip: ViewportClip::HardEdge,
            zoom_policy: ViewportZoomPolicy::WheelWithModifier,
            min_scale: DEFAULT_MIN_VIEWPORT_SCALE,
            max_scale: DEFAULT_MAX_VIEWPORT_SCALE,
            friction: DEFAULT_VIEWPORT_FRICTION,
            on_interaction_start: None,
            on_interaction_update: None,
            on_interaction_end: None,
        }
    }
}

impl InternalLower for InteractiveViewer {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        let (min_scale, max_scale) = normalized_scale_bounds(self.min_scale, self.max_scale);

        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        let mut builder = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::InteractiveViewport {
                initial_transform: clamped_transform(self.initial_transform, min_scale, max_scale),
                controlled_transform: self
                    .transform
                    .map(|transform| clamped_transform(transform, min_scale, max_scale)),
                pan_axis: self.pan_axis,
                boundary: self.boundary.normalized(),
                clip: self.clip,
                zoom_policy: self.zoom_policy,
                min_scale,
                max_scale,
                friction: normalized_friction(self.friction),
                on_interaction_start: action_entry(
                    self.on_interaction_start.as_ref(),
                    ActionTrigger::ViewportInteractionStart,
                ),
                on_interaction_update: action_entry(
                    self.on_interaction_update.as_ref(),
                    ActionTrigger::ViewportInteractionUpdate,
                ),
                on_interaction_end: action_entry(
                    self.on_interaction_end.as_ref(),
                    ActionTrigger::ViewportInteractionEnd,
                ),
            }),
        );
        builder.add_child(child_id);
        builder.build(cx)
    }
}

fn normalized_scale_bounds(min_scale: f32, max_scale: f32) -> (f32, f32) {
    let min_scale = if min_scale.is_finite() && min_scale > 0.0 {
        min_scale
    } else {
        DEFAULT_MIN_VIEWPORT_SCALE
    };
    let max_scale = if max_scale.is_finite() && max_scale > 0.0 {
        max_scale.max(min_scale)
    } else {
        DEFAULT_MAX_VIEWPORT_SCALE.max(min_scale)
    };
    (min_scale, max_scale)
}

fn clamped_transform(
    transform: ViewportTransform,
    min_scale: f32,
    max_scale: f32,
) -> ViewportTransform {
    let mut transform = transform.normalized();
    transform.scale = transform.scale.clamp(min_scale, max_scale);
    transform
}

fn normalized_friction(friction: f32) -> f32 {
    if friction.is_finite() && friction >= 0.0 {
        friction
    } else {
        DEFAULT_VIEWPORT_FRICTION
    }
}

fn action_entry(action: Option<&ActionEnvelope>, trigger: ActionTrigger) -> Option<ActionEntry> {
    action.map(|action| ActionEntry {
        trigger,
        action_id: action.id.as_u128(),
        payload_data: Some(action.payload.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionId;
    use crate::env::{Env, RuntimeState};
    use crate::lowering::InternalLoweringCx;
    use crate::ui::Container;

    #[test]
    fn transform_round_trips_between_world_and_screen() {
        let transform = ViewportTransform::new(24.0, -18.0, 2.5);
        let world = [13.0, -7.0];
        let screen = transform.world_to_screen(world);

        let round_trip = transform.screen_to_world(screen);
        assert!((round_trip[0] - world[0]).abs() < 0.0001);
        assert!((round_trip[1] - world[1]).abs() < 0.0001);
    }

    #[test]
    fn scaling_around_a_focal_point_does_not_move_that_world_point() {
        let transform = ViewportTransform::new(30.0, 40.0, 1.5);
        let focal = [220.0, 140.0];
        let world = transform.screen_to_world(focal);
        let scaled = transform.with_scale_around(focal, 3.0);

        let projected = scaled.world_to_screen(world);
        assert!((projected[0] - focal[0]).abs() < 0.0001);
        assert!((projected[1] - focal[1]).abs() < 0.0001);
    }

    #[test]
    fn lowering_preserves_one_child_configuration_and_action_payloads() {
        let id = WidgetId::explicit("interactive-viewer");
        let start = ActionEnvelope {
            id: ActionId::from_u128(41),
            payload: br#"{"document":"alpha"}"#.to_vec(),
        };
        let update = ActionEnvelope {
            id: ActionId::from_u128(42),
            payload: br#"{"document":"beta"}"#.to_vec(),
        };
        let end = ActionEnvelope {
            id: ActionId::from_u128(43),
            payload: br#"{"document":"gamma"}"#.to_vec(),
        };
        let viewer: Widget = InteractiveViewer {
            id: Some(id),
            child: Container::default().into(),
            initial_transform: ViewportTransform::new(5.0, 7.0, 0.1),
            transform: Some(ViewportTransform::new(11.0, 13.0, 9.0)),
            boundary: ViewportBoundary::finite(
                100.0,
                80.0,
                -100.0,
                -80.0,
                ViewportMargin::all(16.0),
            ),
            min_scale: 0.5,
            max_scale: 4.0,
            on_interaction_start: Some(start.clone()),
            on_interaction_update: Some(update.clone()),
            on_interaction_end: Some(end.clone()),
            ..Default::default()
        }
        .into();

        let env = Env::default();
        let runtime_state = RuntimeState::default();
        let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
        let root_id = viewer.lower(&mut cx);
        let node = cx.ir.nodes.get(&root_id).expect("viewer node");

        assert_eq!(root_id, id);
        assert_eq!(node.children.len(), 1);
        let Op::Layout(LayoutOp::InteractiveViewport {
            initial_transform,
            controlled_transform,
            boundary,
            on_interaction_start,
            on_interaction_update,
            on_interaction_end,
            ..
        }) = &node.op
        else {
            panic!("expected InteractiveViewport, got {:?}", node.op);
        };
        assert_eq!(initial_transform.scale, 0.5);
        assert_eq!(
            controlled_transform.expect("controlled transform").scale,
            4.0
        );
        assert_eq!(
            *boundary,
            ViewportBoundary::finite(-100.0, -80.0, 100.0, 80.0, ViewportMargin::all(16.0),)
        );
        let action = on_interaction_start.as_ref().expect("start action");
        assert_eq!(action.trigger, ActionTrigger::ViewportInteractionStart);
        assert_eq!(action.action_id, start.id.as_u128());
        assert_eq!(
            action.payload_data.as_deref(),
            Some(start.payload.as_slice())
        );
        let action = on_interaction_update.as_ref().expect("update action");
        assert_eq!(action.trigger, ActionTrigger::ViewportInteractionUpdate);
        assert_eq!(action.action_id, update.id.as_u128());
        assert_eq!(
            action.payload_data.as_deref(),
            Some(update.payload.as_slice())
        );
        let action = on_interaction_end.as_ref().expect("end action");
        assert_eq!(action.trigger, ActionTrigger::ViewportInteractionEnd);
        assert_eq!(action.action_id, end.id.as_u128());
        assert_eq!(action.payload_data.as_deref(), Some(end.payload.as_slice()));
    }
}
