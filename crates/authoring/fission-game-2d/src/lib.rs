//! Retained Fission widgets for renderer-independent [`Scene2DIR`] output.
//!
//! The game runtime remains headless. This crate is the graphical adapter that
//! turns the same validated scene into ordinary Fission widgets, preserving
//! layout, accessibility, hit testing, and shell portability.

use std::collections::BTreeMap;

use fission_core::ui::widgets::Transform;
use fission_core::ui::{
    Composite, Container, GestureDetector, Image, Positioned, Pressable, PressableRole,
    PressableStyle, SemanticsRegion, Text, Widget, ZStack,
};
use fission_core::ActionEnvelope;
use fission_game::{
    Anchor, Bounds2D, ImageInstance2D, Scene2DCommand, Scene2DIR, SceneNodeId, Size, Transform2D,
};
use fission_ir::op::{ImageAlignment, ImageFit};

/// Accessible activation attached to one visible scene declaration.
#[derive(Clone, Debug)]
pub struct SceneTapAction {
    /// Localized accessible name for the scene object.
    pub label: String,
    /// Context-preserving application action dispatched on activation.
    pub action: ActionEnvelope,
    /// Whether activation and focus are suppressed while retaining semantics.
    pub disabled: bool,
    /// Optional stable identifier used by semantic tests and automation.
    pub semantics_identifier: Option<String>,
}

impl SceneTapAction {
    pub fn new(label: impl Into<String>, action: ActionEnvelope) -> Self {
        Self {
            label: label.into(),
            action,
            disabled: false,
            semantics_identifier: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

/// Semantic interactions attached to one retained scene object.
///
/// The action payloads normally contain the object's durable domain identity;
/// live pointer coordinates and deltas remain in `ReducerContext::input`.
/// Disabled declarations keep the object visible but suppress every action.
#[derive(Clone, Debug)]
pub struct SceneObjectActions {
    /// Localized accessible name for the scene object.
    pub label: String,
    /// Action dispatched for ordinary pointer, keyboard, accessibility, and
    /// LiveTest activation.
    pub on_tap: Option<ActionEnvelope>,
    /// Action dispatched once when an object drag crosses Fission's gesture
    /// threshold.
    pub on_drag_start: Option<ActionEnvelope>,
    /// Action dispatched for subsequent drag movement.
    pub on_drag_update: Option<ActionEnvelope>,
    /// Action dispatched when a captured drag finishes.
    pub on_drag_end: Option<ActionEnvelope>,
    /// Action dispatched when the platform cancels a captured drag.
    ///
    /// When present, cancellation dispatches only this action; ordinary
    /// pointer release continues to dispatch only `on_drag_end`.
    ///
    /// If omitted, `on_drag_end` remains the compatibility fallback for both
    /// release and cancellation.
    pub on_drag_cancel: Option<ActionEnvelope>,
    /// Action dispatched after the platform long-press threshold.
    pub on_long_press: Option<ActionEnvelope>,
    /// Whether all interaction and focus are suppressed.
    pub disabled: bool,
    /// Optional stable identifier used by semantic tests and automation.
    pub semantics_identifier: Option<String>,
}

impl SceneObjectActions {
    /// Creates an interaction declaration with a localized accessible label
    /// and no actions. Add only the gestures the object supports.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            on_tap: None,
            on_drag_start: None,
            on_drag_update: None,
            on_drag_end: None,
            on_drag_cancel: None,
            on_long_press: None,
            disabled: false,
            semantics_identifier: None,
        }
    }

    /// Dispatches `action` when the object is activated through pointer,
    /// keyboard, accessibility, or LiveTest input.
    pub fn on_tap(mut self, action: ActionEnvelope) -> Self {
        self.on_tap = Some(action);
        self
    }

    /// Dispatches `action` once a pointer movement becomes a drag.
    pub fn on_drag_start(mut self, action: ActionEnvelope) -> Self {
        self.on_drag_start = Some(action);
        self
    }

    /// Dispatches `action` for each movement after drag capture. The reducer
    /// reads the current point and incremental delta from its action input.
    pub fn on_drag_update(mut self, action: ActionEnvelope) -> Self {
        self.on_drag_update = Some(action);
        self
    }

    /// Dispatches `action` when the captured drag ends.
    pub fn on_drag_end(mut self, action: ActionEnvelope) -> Self {
        self.on_drag_end = Some(action);
        self
    }

    /// Dispatches `action` when a captured drag is interrupted rather than
    /// released normally. Its bound payload is preserved; the live pointer
    /// position remains available through the reducer action input.
    pub fn on_drag_cancel(mut self, action: ActionEnvelope) -> Self {
        self.on_drag_cancel = Some(action);
        self
    }

    /// Dispatches `action` after Fission recognizes a long press.
    pub fn on_long_press(mut self, action: ActionEnvelope) -> Self {
        self.on_long_press = Some(action);
        self
    }

    /// Keeps the object visible and described semantically while suppressing
    /// all configured gestures and focus.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets a stable identifier for accessibility and LiveTest selectors.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<SceneTapAction> for SceneObjectActions {
    fn from(tap: SceneTapAction) -> Self {
        Self {
            label: tap.label,
            on_tap: Some(tap.action),
            disabled: tap.disabled,
            semantics_identifier: tap.semantics_identifier,
            on_drag_start: None,
            on_drag_update: None,
            on_drag_end: None,
            on_drag_cancel: None,
            on_long_press: None,
        }
    }
}

/// Graphical adapter for a validated renderer-independent 2D scene.
///
/// Scene declarations become ordinary retained widgets rather than a private
/// renderer overlay. As a result pointer, keyboard, accessibility, and test
/// activation use Fission's standard `Pressable` contract.
#[derive(Clone, Debug)]
pub struct Scene2DView {
    pub scene: Scene2DIR,
    pub width: f32,
    pub height: f32,
    interactions: BTreeMap<SceneNodeId, SceneObjectActions>,
}

impl Scene2DView {
    pub fn new(scene: Scene2DIR, width: f32, height: f32) -> Self {
        Self {
            scene,
            width: finite_non_negative(width),
            height: finite_non_negative(height),
            interactions: BTreeMap::new(),
        }
    }

    /// Makes one scene object directly activatable through every standard
    /// Fission input route. `label` must already be localized by the app.
    pub fn on_tap(
        mut self,
        id: SceneNodeId,
        label: impl Into<String>,
        action: ActionEnvelope,
    ) -> Self {
        self.interactions
            .insert(id, SceneObjectActions::new(label).on_tap(action));
        self
    }

    /// Attaches a complete activation declaration, including disabled state
    /// and a stable semantic-test identifier.
    pub fn tap_action(mut self, id: SceneNodeId, tap: SceneTapAction) -> Self {
        self.interactions.insert(id, tap.into());
        self
    }

    /// Attaches tap, drag, and long-press behavior to one retained scene
    /// object without introducing a renderer-specific input path.
    pub fn object_actions(mut self, id: SceneNodeId, actions: SceneObjectActions) -> Self {
        self.interactions.insert(id, actions);
        self
    }
}

impl From<Scene2DView> for Widget {
    fn from(view: Scene2DView) -> Self {
        let mut children = Vec::new();
        for command in view.scene.commands {
            append_command(&mut children, command, &view.interactions);
        }

        Container::new(ZStack {
            children,
            ..Default::default()
        })
        .size(view.width, view.height)
        .into()
    }
}

fn append_command(
    children: &mut Vec<Widget>,
    command: Scene2DCommand,
    interactions: &BTreeMap<SceneNodeId, SceneObjectActions>,
) {
    match command {
        Scene2DCommand::Clear { color } => children.push(
            Positioned {
                left: Some(0.0),
                top: Some(0.0),
                right: Some(0.0),
                bottom: Some(0.0),
                child: Some(Container::default().bg(color).into()),
                ..Default::default()
            }
            .into(),
        ),
        Scene2DCommand::DrawRect {
            id,
            bounds,
            fill,
            opacity,
            ..
        } => {
            let visual: Widget = Container::default().bg(fill).into();
            children.push(positioned(
                id.clone(),
                bounds,
                with_interaction(id, with_opacity(visual, opacity), interactions),
            ));
        }
        Scene2DCommand::DrawImage {
            id,
            image,
            transform,
            size,
            opacity,
            ..
        } => children.push(image_widget(
            id,
            image.request,
            transform,
            size,
            opacity,
            interactions,
        )),
        Scene2DCommand::DrawText {
            id,
            text,
            transform,
            size,
            color,
            opacity,
            ..
        } => {
            let visual: Widget = Text::new(text).size(size.0).color(color).into();
            let visual = transformed(with_opacity(visual, opacity), transform, Size::default());
            let bounds = Bounds2D::from_top_left(transform.translation, Size::default());
            children.push(positioned(
                id.clone(),
                bounds,
                with_interaction(id, visual, interactions),
            ));
        }
        Scene2DCommand::ImageBatch {
            image, instances, ..
        } => {
            for ImageInstance2D {
                id,
                transform,
                size,
                opacity,
            } in instances
            {
                children.push(image_widget(
                    id,
                    image.request.clone(),
                    transform,
                    size,
                    opacity,
                    interactions,
                ));
            }
        }
    }
}

fn image_widget(
    id: SceneNodeId,
    request: fission_ir::op::ImageRequest,
    transform: Transform2D,
    size: Size,
    opacity: f32,
    interactions: &BTreeMap<SceneNodeId, SceneObjectActions>,
) -> Widget {
    let visual: Widget = Image {
        request,
        width: Some(size.width.0),
        height: Some(size.height.0),
        fit: ImageFit::Contain,
        alignment: ImageAlignment::Center,
        ..Default::default()
    }
    .into();
    let visual = transformed(with_opacity(visual, opacity), transform, size);
    let bounds = placement_bounds(transform, size);
    positioned(
        id.clone(),
        bounds,
        with_interaction(id, visual, interactions),
    )
}

fn with_interaction(
    id: SceneNodeId,
    visual: Widget,
    interactions: &BTreeMap<SceneNodeId, SceneObjectActions>,
) -> Widget {
    let retained_id = id.widget_id();
    let Some(actions) = interactions.get(&id) else {
        return Container {
            id: Some(retained_id),
            child: Some(visual),
            ..Default::default()
        }
        .into();
    };

    let identifier = actions
        .semantics_identifier
        .clone()
        .or_else(|| Some(format!("game.scene.{}", retained_id.as_u128())));

    // Keep gesture semantics outside the activation semantics. Hit testing may
    // legitimately stop at the Pressable itself when the visual is transparent
    // or non-painting. Gesture dispatch walks from that hit node toward its
    // ancestors, so nesting the GestureDetector inside the Pressable would make
    // drag and long-press actions unreachable for those scene objects.
    let activation_visual: Widget = if let Some(on_tap) = &actions.on_tap {
        Pressable {
            id: Some(retained_id),
            child: visual,
            on_press: Some(on_tap.clone()),
            label: Some(actions.label.clone()),
            semantics_identifier: identifier,
            role: PressableRole::Button,
            disabled: actions.disabled,
            hover_style: Some(PressableStyle {
                scale: Some(1.08),
                ..Default::default()
            }),
            pressed_style: Some(PressableStyle {
                scale: Some(0.94),
                ..Default::default()
            }),
            ..Default::default()
        }
        .into()
    } else {
        let mut region = SemanticsRegion::new(visual)
            .label(actions.label.clone())
            .role(fission_ir::semantics::Role::Generic);
        region.id = Some(retained_id);
        region.identifier = identifier;
        region.into()
    };

    GestureDetector {
        child: activation_visual,
        on_drag_start: (!actions.disabled)
            .then(|| actions.on_drag_start.clone())
            .flatten(),
        on_drag_update: (!actions.disabled)
            .then(|| actions.on_drag_update.clone())
            .flatten(),
        on_drag_end: (!actions.disabled)
            .then(|| actions.on_drag_end.clone())
            .flatten(),
        on_drag_cancel: (!actions.disabled)
            .then(|| actions.on_drag_cancel.clone())
            .flatten(),
        on_long_press: (!actions.disabled)
            .then(|| actions.on_long_press.clone())
            .flatten(),
        ..Default::default()
    }
    .into()
}

fn positioned(_id: SceneNodeId, bounds: Bounds2D, child: Widget) -> Widget {
    Positioned {
        left: Some(bounds.min.x.0),
        top: Some(bounds.min.y.0),
        width: (bounds.width().0 > 0.0).then_some(bounds.width().0),
        height: (bounds.height().0 > 0.0).then_some(bounds.height().0),
        child: Some(child),
        ..Default::default()
    }
    .into()
}

fn with_opacity(child: Widget, opacity: f32) -> Widget {
    if opacity < 1.0 {
        Composite::new(child)
            .opacity(opacity.clamp(0.0, 1.0))
            .into()
    } else {
        child
    }
}

fn transformed(child: Widget, transform: Transform2D, size: Size) -> Widget {
    if transform.rotation.0 == 0.0 && transform.scale_x == 1.0 && transform.scale_y == 1.0 {
        return child;
    }
    Transform::new(child, transform_matrix(transform, size)).into()
}

fn placement_bounds(transform: Transform2D, size: Size) -> Bounds2D {
    match transform.anchor {
        Anchor::Center => Bounds2D::from_center(transform.translation, size),
        Anchor::TopLeft => Bounds2D::from_top_left(transform.translation, size),
    }
}

fn transform_matrix(transform: Transform2D, size: Size) -> [f32; 16] {
    let radians = transform.rotation.0.to_radians();
    let (sin, cos) = radians.sin_cos();
    let (pivot_x, pivot_y) = match transform.anchor {
        Anchor::Center => (size.width.0 / 2.0, size.height.0 / 2.0),
        Anchor::TopLeft => (0.0, 0.0),
    };
    let a = cos * transform.scale_x;
    let b = sin * transform.scale_x;
    let c = -sin * transform.scale_y;
    let d = cos * transform.scale_y;
    let tx = pivot_x - a * pivot_x - c * pivot_y;
    let ty = pivot_y - b * pivot_x - d * pivot_y;
    [
        a, b, 0.0, 0.0, c, d, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0,
    ]
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use fission_game::{Layer, Place, Px};
    use fission_ir::{
        op::{Color, Op},
        semantics::ActionTrigger,
    };

    use super::*;

    #[test]
    fn scene_objects_lower_as_retained_visuals_with_standard_actions() {
        let object = SceneNodeId::from_key(&7_u32);
        let action = ActionEnvelope {
            id: fission_core::ActionId::from_name("catch"),
            payload: vec![1, 2],
        };
        let mut scene = fission_game::Scene2D::new();
        scene.rect(
            object.clone(),
            Bounds2D::from_top_left(
                Place::new(Px(10.0), Px(20.0)),
                Size::new(Px(30.0), Px(40.0)),
            ),
            Color::BLUE,
            Layer(1),
        );
        let widget: Widget = Scene2DView::new(scene.finish(fission_game::Tick(2)), 100.0, 80.0)
            .on_tap(object.clone(), "Catch fish", action.clone())
            .into();

        let ir = fission_core::internal::lower_widget_to_ir(&widget);

        let semantic = ir
            .nodes
            .values()
            .find_map(|node| match &node.op {
                Op::Semantics(semantics) if node.id == object.widget_id() => Some(semantics),
                _ => None,
            })
            .expect("scene object should lower to a retained semantic action");
        assert_eq!(semantic.label.as_deref(), Some("Catch fish"));
        assert!(semantic.actions.entries.iter().any(|entry| {
            entry.action_id == action.id.as_u128()
                && entry.payload_data == Some(action.payload.clone())
        }));
    }

    #[test]
    fn multiple_interactive_images_keep_visual_and_action_identities_distinct() {
        let first = SceneNodeId::from_key(&101_u32);
        let second = SceneNodeId::from_key(&102_u32);
        let action = ActionEnvelope {
            id: fission_core::ActionId::from_name("catch"),
            payload: Vec::new(),
        };
        let mut scene = fission_game::Scene2D::new();
        scene.image(
            first.clone(),
            fission_game::ImageAsset::asset(&101_u32, "fish-blue.png", 192, 192),
            Transform2D::at(Place::new(Px(60.0), Px(80.0))),
            Size::new(Px(48.0), Px(48.0)),
            Layer(1),
        );
        scene.image(
            second.clone(),
            fission_game::ImageAsset::asset(&102_u32, "fish-orange.png", 192, 192),
            Transform2D::at(Place::new(Px(180.0), Px(120.0))),
            Size::new(Px(48.0), Px(48.0)),
            Layer(1),
        );

        let widget: Widget = Scene2DView::new(scene.finish(fission_game::Tick(0)), 240.0, 180.0)
            .on_tap(first.clone(), "Catch blue fish", action.clone())
            .on_tap(second.clone(), "Catch orange fish", action)
            .into();
        let ir = fission_core::internal::lower_widget_to_ir(&widget);

        assert!(ir.nodes.contains_key(&first.widget_id()));
        assert!(ir.nodes.contains_key(&second.widget_id()));
        assert_ne!(first.widget_id(), second.widget_id());
    }

    #[test]
    fn scene_object_actions_preserve_drag_contract_and_payloads() {
        let object = SceneNodeId::from_key(&88_u32);
        let drag_start = ActionEnvelope {
            id: fission_core::ActionId::from_name("scene-drag-start"),
            payload: vec![1, 3, 5],
        };
        let drag_update = ActionEnvelope {
            id: fission_core::ActionId::from_name("scene-drag-update"),
            payload: vec![2, 4, 6],
        };
        let drag_end = ActionEnvelope {
            id: fission_core::ActionId::from_name("scene-drag-end"),
            payload: vec![7, 8, 9],
        };
        let drag_cancel = ActionEnvelope {
            id: fission_core::ActionId::from_name("scene-drag-cancel"),
            payload: vec![10, 11, 12],
        };
        let mut scene = fission_game::Scene2D::new();
        scene.rect(
            object.clone(),
            Bounds2D::from_top_left(Place::new(Px(8.0), Px(12.0)), Size::new(Px(36.0), Px(28.0))),
            Color::BLUE,
            Layer(1),
        );

        let widget: Widget = Scene2DView::new(scene.finish(fission_game::Tick(0)), 120.0, 90.0)
            .object_actions(
                object.clone(),
                SceneObjectActions::new("Move survivor")
                    .on_drag_start(drag_start.clone())
                    .on_drag_update(drag_update.clone())
                    .on_drag_end(drag_end.clone())
                    .on_drag_cancel(drag_cancel.clone())
                    .semantics_identifier("game.scene.survivor"),
            )
            .into();
        let ir = fission_core::internal::lower_widget_to_ir(&widget);

        let semantic_nodes = ir
            .nodes
            .values()
            .filter_map(|node| match &node.op {
                Op::Semantics(semantics) => Some((node.id, semantics)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(semantic_nodes.iter().any(|(id, semantics)| {
            *id == object.widget_id()
                && semantics.identifier.as_deref() == Some("game.scene.survivor")
                && semantics.label.as_deref() == Some("Move survivor")
        }));

        let assert_action = |trigger, expected: &ActionEnvelope| {
            assert!(semantic_nodes.iter().any(|(_, semantics)| {
                semantics.actions.entries.iter().any(|entry| {
                    entry.trigger == trigger
                        && entry.action_id == expected.id.as_u128()
                        && entry.payload_data.as_ref() == Some(&expected.payload)
                })
            }));
        };
        assert_action(ActionTrigger::DragStart, &drag_start);
        assert_action(ActionTrigger::DragUpdate, &drag_update);
        assert_action(ActionTrigger::DragEnd, &drag_end);
        assert_action(ActionTrigger::DragCancel, &drag_cancel);
    }

    #[test]
    fn tap_and_drag_actions_share_a_reachable_path_for_transparent_objects() {
        let object = SceneNodeId::from_key(&188_u32);
        let tap = ActionEnvelope {
            id: fission_core::ActionId::from_name("transparent-tap"),
            payload: vec![1],
        };
        let drag = ActionEnvelope {
            id: fission_core::ActionId::from_name("transparent-drag"),
            payload: vec![2],
        };
        let mut scene = fission_game::Scene2D::new();
        scene.rect(
            object.clone(),
            Bounds2D::from_top_left(
                Place::new(Px(10.0), Px(10.0)),
                Size::new(Px(44.0), Px(44.0)),
            ),
            Color::TRANSPARENT,
            Layer(1),
        );
        let widget: Widget = Scene2DView::new(scene.finish(fission_game::Tick(0)), 80.0, 80.0)
            .object_actions(
                object.clone(),
                SceneObjectActions::new("Transparent control")
                    .on_tap(tap.clone())
                    .on_drag_start(drag.clone())
                    .on_drag_update(drag.clone()),
            )
            .into();
        let ir = fission_core::internal::lower_widget_to_ir(&widget);

        let activation_id = object.widget_id();
        let activation_node = ir
            .nodes
            .get(&activation_id)
            .expect("scene identity should remain on the activation semantics");
        let Op::Semantics(activation) = &activation_node.op else {
            panic!("scene identity should lower to semantics");
        };
        assert!(activation.actions.entries.iter().any(|entry| {
            entry.trigger == ActionTrigger::Default
                && entry.action_id == tap.id.as_u128()
                && entry.payload_data.as_ref() == Some(&tap.payload)
        }));

        let mut ancestor = activation_node.parent;
        let mut drag_is_reachable = false;
        while let Some(id) = ancestor {
            let node = ir.nodes.get(&id).expect("ancestor must exist");
            if let Op::Semantics(semantics) = &node.op {
                drag_is_reachable = semantics.actions.entries.iter().any(|entry| {
                    entry.trigger == ActionTrigger::DragStart
                        && entry.action_id == drag.id.as_u128()
                        && entry.payload_data.as_ref() == Some(&drag.payload)
                });
                if drag_is_reachable {
                    break;
                }
            }
            ancestor = node.parent;
        }
        assert!(
            drag_is_reachable,
            "gesture dispatch starts at the activation hit and walks ancestors"
        );
    }

    #[test]
    fn disabled_scene_object_suppresses_every_interaction() {
        let object = SceneNodeId::from_key(&99_u32);
        let action = ActionEnvelope {
            id: fission_core::ActionId::from_name("disabled-scene-action"),
            payload: vec![42],
        };
        let mut scene = fission_game::Scene2D::new();
        scene.rect(
            object.clone(),
            Bounds2D::from_top_left(Place::new(Px(0.0), Px(0.0)), Size::new(Px(20.0), Px(20.0))),
            Color::BLUE,
            Layer(1),
        );

        let widget: Widget = Scene2DView::new(scene.finish(fission_game::Tick(0)), 40.0, 40.0)
            .object_actions(
                object,
                SceneObjectActions::new("Unavailable salvage")
                    .on_tap(action.clone())
                    .on_drag_start(action.clone())
                    .on_drag_update(action.clone())
                    .on_drag_end(action.clone())
                    .on_drag_cancel(action)
                    .disabled(true),
            )
            .into();
        let ir = fission_core::internal::lower_widget_to_ir(&widget);

        assert!(ir.nodes.values().all(|node| match &node.op {
            Op::Semantics(semantics) => semantics.actions.entries.is_empty(),
            _ => true,
        }));
    }

    #[test]
    fn center_transform_rotates_and_scales_around_the_sprite_center() {
        let matrix = transform_matrix(
            Transform2D {
                rotation: fission_game::Degrees(90.0),
                scale_x: 2.0,
                scale_y: 1.0,
                ..Default::default()
            },
            Size::new(Px(20.0), Px(10.0)),
        );
        assert!((matrix[12] - 15.0).abs() < 0.001);
        assert!((matrix[13] + 15.0).abs() < 0.001);
    }
}
