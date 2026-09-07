//! Retained Fission widgets for renderer-independent [`Scene2DIR`] output.
//!
//! The game runtime remains headless. This crate is the graphical adapter that
//! turns the same validated scene into ordinary Fission widgets, preserving
//! layout, accessibility, hit testing, and shell portability.

mod scene_path;
mod scene_visual;

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

use scene_path::ScenePathLayer;
use scene_visual::{ActionlessSceneVisual, SceneObjectSemantics};

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
/// Disabled declarations keep the object visible and described semantically,
/// but suppress every action and do not claim coordinate hits.
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

    fn has_pointer_gesture(&self) -> bool {
        self.on_tap.is_some()
            || self.on_drag_start.is_some()
            || self.on_drag_update.is_some()
            || self.on_drag_end.is_some()
            || self.on_drag_cancel.is_some()
            || self.on_long_press.is_some()
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
/// activation use Fission's standard `Pressable` contract. Actionless scene
/// declarations are pointer-transparent; attach at least one enabled pointer
/// gesture through `on_tap` or `object_actions` to make a declaration's bounds
/// interactive.
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
        let camera_origin = view
            .scene
            .camera_bounds
            .map(|bounds| bounds.min)
            .unwrap_or_default();
        let mut children = Vec::new();
        for command in view.scene.commands {
            append_command(&mut children, command, camera_origin, &view.interactions);
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
    camera_origin: fission_game::Place,
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
            let bounds = viewport_bounds(bounds, camera_origin);
            let visual: Widget = Container::default()
                .bg(fill)
                .size(bounds.width().0, bounds.height().0)
                .into();
            children.push(positioned(
                id.clone(),
                bounds,
                with_interaction(id, with_opacity(visual, opacity), interactions),
            ));
        }
        Scene2DCommand::DrawImage {
            id,
            image,
            mut transform,
            size,
            opacity,
            ..
        } => {
            transform.translation = viewport_place(transform.translation, camera_origin);
            children.push(image_widget(
                id,
                image.request,
                transform,
                size,
                opacity,
                interactions,
            ));
        }
        Scene2DCommand::DrawText {
            id,
            text,
            mut transform,
            size,
            color,
            opacity,
            ..
        } => {
            transform.translation = viewport_place(transform.translation, camera_origin);
            let visual: Widget = Text::new(text).size(size.0).color(color).into();
            let visual = transformed(with_opacity(visual, opacity), transform, Size::default());
            let bounds = Bounds2D::from_top_left(transform.translation, Size::default());
            children.push(positioned(
                id.clone(),
                bounds,
                with_interaction(id, visual, interactions),
            ));
        }
        Scene2DCommand::DrawPath {
            id,
            path,
            bounds,
            fill,
            stroke,
            opacity,
            ..
        } => {
            let bounds = viewport_bounds(bounds, camera_origin);
            let visual: Widget = ScenePathLayer::new(
                &id,
                path,
                bounds.width().0,
                bounds.height().0,
                fill,
                stroke,
                opacity,
            )
            .into();
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
                mut transform,
                size,
                opacity,
            } in instances
            {
                transform.translation = viewport_place(transform.translation, camera_origin);
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

fn viewport_place(
    place: fission_game::Place,
    camera_origin: fission_game::Place,
) -> fission_game::Place {
    fission_game::Place::new(place.x - camera_origin.x, place.y - camera_origin.y)
}

fn viewport_bounds(bounds: Bounds2D, camera_origin: fission_game::Place) -> Bounds2D {
    Bounds2D {
        min: viewport_place(bounds.min, camera_origin),
        max: viewport_place(bounds.max, camera_origin),
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
        return ActionlessSceneVisual::new(retained_id, visual).into();
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
        SceneObjectSemantics::new(
            retained_id,
            actions.label.clone(),
            identifier,
            actions.disabled,
            visual,
        )
        .into()
    };

    let interaction_visual: Widget = GestureDetector {
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
    .into();

    if actions.disabled || !actions.has_pointer_gesture() {
        ActionlessSceneVisual::preserving_child_identity(retained_id, interaction_visual).into()
    } else {
        interaction_visual
    }
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
        op::{Color, Fill as IrFill, Op},
        semantics::ActionTrigger,
    };
    use fission_render::{Color as RenderColor, DisplayOp, Fill};
    use fission_test::{TestDriver, TestHarness};

    use super::*;

    #[test]
    fn actionless_rect_paints_at_its_declared_scene_bounds() {
        let expected_color = RenderColor {
            r: 23,
            g: 117,
            b: 203,
            a: 231,
        };
        let mut scene = fission_game::Scene2D::new();
        scene.rect(
            SceneNodeId::from_key(&31_u32),
            Bounds2D::from_top_left(
                Place::new(Px(13.0), Px(17.0)),
                Size::new(Px(31.0), Px(23.0)),
            ),
            Color {
                r: expected_color.r,
                g: expected_color.g,
                b: expected_color.b,
                a: expected_color.a,
            },
            Layer(4),
        );
        let view = Scene2DView::new(scene.finish(fission_game::Tick(0)), 100.0, 80.0);
        let mut harness = TestHarness::new_with_mock_measurer(()).with_root_widget(view);

        harness.pump().expect("scene should render");

        let painted = harness
            .get_last_display_list()
            .expect("rendered display list")
            .ops
            .into_iter()
            .find_map(|op| match op {
                DisplayOp::DrawRect {
                    rect,
                    fill: Some(Fill::Solid(color)),
                    ..
                } if color == expected_color => Some(rect),
                _ => None,
            })
            .expect("declared scene rectangle should reach the display list");
        assert!((painted.x() - 13.0).abs() < 0.01, "{painted:?}");
        assert!((painted.y() - 17.0).abs() < 0.01, "{painted:?}");
        assert!((painted.width() - 31.0).abs() < 0.01, "{painted:?}");
        assert!((painted.height() - 23.0).abs() < 0.01, "{painted:?}");
    }

    #[test]
    fn camera_origin_translates_world_coordinates_into_the_viewport() {
        let expected_color = RenderColor {
            r: 11,
            g: 99,
            b: 188,
            a: 255,
        };
        let mut scene = fission_game::Scene2D::new();
        scene.camera_bounds(Bounds2D::from_top_left(
            Place::new(Px(100.0), Px(50.0)),
            Size::new(Px(80.0), Px(60.0)),
        ));
        scene.rect(
            SceneNodeId::from_key(&41_u32),
            Bounds2D::from_top_left(
                Place::new(Px(112.0), Px(57.0)),
                Size::new(Px(20.0), Px(10.0)),
            ),
            Color {
                r: expected_color.r,
                g: expected_color.g,
                b: expected_color.b,
                a: expected_color.a,
            },
            Layer(1),
        );
        let view = Scene2DView::new(scene.finish(fission_game::Tick(0)), 80.0, 60.0);
        let mut harness = TestHarness::new_with_mock_measurer(()).with_root_widget(view);

        harness.pump().expect("camera-offset scene should render");

        let painted = harness
            .get_last_display_list()
            .expect("rendered display list")
            .ops
            .into_iter()
            .find_map(|op| match op {
                DisplayOp::DrawRect {
                    rect,
                    fill: Some(Fill::Solid(color)),
                    ..
                } if color == expected_color => Some(rect),
                _ => None,
            })
            .expect("camera-visible rectangle should reach the display list");
        assert!((painted.x() - 12.0).abs() < 0.01, "{painted:?}");
        assert!((painted.y() - 7.0).abs() < 0.01, "{painted:?}");
    }

    #[test]
    fn interactive_path_paints_inside_its_clipped_bounds_with_group_opacity() {
        let object = SceneNodeId::from_key(&32_u32);
        let action = ActionEnvelope {
            id: fission_core::ActionId::from_name("select-path"),
            payload: vec![3, 2],
        };
        let bounds = Bounds2D::from_top_left(
            Place::new(Px(13.0), Px(17.0)),
            Size::new(Px(31.0), Px(23.0)),
        );
        let path = "M-8 -8 L40 10 L12 31 Z";
        let expected_color = RenderColor {
            r: 18,
            g: 132,
            b: 211,
            a: 255,
        };
        let mut scene = fission_game::Scene2D::new();
        scene.command(Scene2DCommand::DrawPath {
            id: object.clone(),
            path: path.into(),
            bounds,
            fill: Some(IrFill::Solid(Color {
                r: expected_color.r,
                g: expected_color.g,
                b: expected_color.b,
                a: expected_color.a,
            })),
            stroke: None,
            layer: Layer(4),
            opacity: 0.4,
        });
        let view = Scene2DView::new(scene.finish(fission_game::Tick(0)), 100.0, 80.0).on_tap(
            object.clone(),
            "Select path",
            action.clone(),
        );

        let widget: Widget = view.clone().into();
        let ir = fission_core::internal::lower_widget_to_ir(&widget);
        let semantic = ir
            .nodes
            .get(&object.widget_id())
            .expect("path scene identity should belong to its interaction wrapper");
        let Op::Semantics(semantics) = &semantic.op else {
            panic!("path scene identity should lower as interaction semantics");
        };
        assert_eq!(semantics.label.as_deref(), Some("Select path"));
        assert!(semantics.actions.entries.iter().any(|entry| {
            entry.action_id == action.id.as_u128()
                && entry.payload_data.as_ref() == Some(&action.payload)
        }));
        assert!(ir.nodes.values().any(|node| {
            matches!(&node.op, Op::Paint(fission_ir::PaintOp::DrawPath { .. }))
                && node.id != object.widget_id()
        }));

        let mut harness = TestHarness::new_with_mock_measurer(()).with_root_widget(view);
        harness.pump().expect("path scene should render");
        let display = harness
            .get_last_display_list()
            .expect("rendered display list");
        let painted = display
            .ops
            .iter()
            .find_map(|op| match op {
                DisplayOp::DrawPath {
                    path: actual_path,
                    fill: Some(Fill::Solid(color)),
                    bounds,
                    ..
                } if actual_path == path && color == &expected_color => Some(*bounds),
                _ => None,
            })
            .expect("declared path should reach the display list unchanged");
        assert!((painted.x() - 13.0).abs() < 0.01, "{painted:?}");
        assert!((painted.y() - 17.0).abs() < 0.01, "{painted:?}");
        assert!((painted.width() - 31.0).abs() < 0.01, "{painted:?}");
        assert!((painted.height() - 23.0).abs() < 0.01, "{painted:?}");
        assert!(display.ops.iter().any(|op| matches!(
            op,
            DisplayOp::ClipRect(rect)
                if (rect.x() - 13.0).abs() < 0.01
                    && (rect.y() - 17.0).abs() < 0.01
                    && (rect.width() - 31.0).abs() < 0.01
                    && (rect.height() - 23.0).abs() < 0.01
        )));
        assert!(display.ops.iter().any(|op| matches!(
            op,
            DisplayOp::OpacityLayer { alpha, bounds }
                if (*alpha - 0.4).abs() < 0.001
                    && (bounds.x() - 13.0).abs() < 0.01
                    && (bounds.y() - 17.0).abs() < 0.01
                    && (bounds.width() - 31.0).abs() < 0.01
                    && (bounds.height() - 23.0).abs() < 0.01
        )));
        assert_eq!(
            semantic_label_at(&harness, fission_core::LayoutPoint::new(28.0, 28.0)).as_deref(),
            Some("Select path"),
            "an interactive path must remain hittable through its semantic owner"
        );
    }

    #[test]
    fn decorative_scene_paint_does_not_intercept_a_lower_coordinate_target() {
        let target = SceneNodeId::from_key(&33_u32);
        let path_decoration = SceneNodeId::from_key(&34_u32);
        let rect_decoration = SceneNodeId::from_key(&35_u32);
        let action = ActionEnvelope {
            id: fission_core::ActionId::from_name("activate-lower-target"),
            payload: Vec::new(),
        };
        let mut scene = fission_game::Scene2D::new();
        scene.rect(
            target.clone(),
            Bounds2D::from_top_left(
                Place::new(Px(18.0), Px(16.0)),
                Size::new(Px(36.0), Px(32.0)),
            ),
            Color::BLUE,
            Layer(1),
        );
        scene.command(Scene2DCommand::DrawPath {
            id: path_decoration,
            path: "M0 0 L80 0 L80 64 L0 64 Z".into(),
            bounds: Bounds2D::from_top_left(
                Place::new(Px(0.0), Px(0.0)),
                Size::new(Px(80.0), Px(64.0)),
            ),
            fill: Some(IrFill::Solid(Color {
                r: 20,
                g: 90,
                b: 140,
                a: 96,
            })),
            stroke: None,
            layer: Layer(2),
            opacity: 1.0,
        });
        let fog_color = Color {
            r: 220,
            g: 230,
            b: 240,
            a: 72,
        };
        scene.command(Scene2DCommand::DrawRect {
            id: rect_decoration,
            bounds: Bounds2D::from_top_left(
                Place::new(Px(0.0), Px(0.0)),
                Size::new(Px(80.0), Px(64.0)),
            ),
            fill: fog_color,
            layer: Layer(3),
            opacity: 1.0,
        });
        let view = Scene2DView::new(scene.finish(fission_game::Tick(0)), 80.0, 64.0).on_tap(
            target,
            "Lower coordinate target",
            action,
        );
        let root: Widget = SemanticsRegion::new(view).label("Decorated scene").into();
        let mut harness = TestHarness::new_with_mock_measurer(()).with_root_widget(root);
        harness.pump().expect("overlaid scene should render");

        let display = harness
            .get_last_display_list()
            .expect("rendered display list");
        assert!(display.ops.iter().any(|op| matches!(
            op,
            DisplayOp::DrawPath { path, .. }
                if path == "M0 0 L80 0 L80 64 L0 64 Z"
        )));
        let expected_fog_color = RenderColor {
            r: fog_color.r,
            g: fog_color.g,
            b: fog_color.b,
            a: fog_color.a,
        };
        assert!(display.ops.iter().any(|op| matches!(
            op,
            DisplayOp::DrawRect {
                fill: Some(Fill::Solid(color)),
                ..
            } if color == &expected_fog_color
        )));

        assert_eq!(
            semantic_label_at(&harness, fission_core::LayoutPoint::new(36.0, 32.0)).as_deref(),
            Some("Lower coordinate target"),
            "actionless scene paint must not claim its rectangular bounds"
        );
    }

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
                SceneObjectActions::new("Move scene object")
                    .on_drag_start(drag_start.clone())
                    .on_drag_update(drag_update.clone())
                    .on_drag_end(drag_end.clone())
                    .on_drag_cancel(drag_cancel.clone())
                    .semantics_identifier("demo.scene.movable"),
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
                && semantics.identifier.as_deref() == Some("demo.scene.movable")
                && semantics.label.as_deref() == Some("Move scene object")
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
                SceneObjectActions::new("Unavailable object")
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
    fn disabled_and_gestureless_scene_metadata_does_not_intercept_a_coordinate_target() {
        let target = SceneNodeId::from_key(&211_u32);
        let gestureless_proxy = SceneNodeId::from_key(&212_u32);
        let disabled_proxy = SceneNodeId::from_key(&213_u32);
        let long_press_target = SceneNodeId::from_key(&214_u32);
        let drag_target = SceneNodeId::from_key(&215_u32);
        let disabled_non_tap_proxy = SceneNodeId::from_key(&216_u32);
        let target_action = ActionEnvelope {
            id: fission_core::ActionId::from_name("tap-lower-scene-target"),
            payload: vec![2, 1, 1],
        };
        let disabled_action = ActionEnvelope {
            id: fission_core::ActionId::from_name("disabled-scene-target"),
            payload: vec![2, 1, 3],
        };
        let long_press_action = ActionEnvelope {
            id: fission_core::ActionId::from_name("hold-scene-target"),
            payload: vec![2, 1, 4],
        };
        let disabled_non_tap_action = ActionEnvelope {
            id: fission_core::ActionId::from_name("disabled-hold-scene-target"),
            payload: vec![2, 1, 6],
        };
        let drag_action = ActionEnvelope {
            id: fission_core::ActionId::from_name("drag-scene-target"),
            payload: vec![2, 1, 5],
        };
        let bounds = Bounds2D::from_top_left(
            Place::new(Px(12.0), Px(14.0)),
            Size::new(Px(48.0), Px(44.0)),
        );
        let mut scene = fission_game::Scene2D::new();
        scene.rect(target.clone(), bounds, Color::BLUE, Layer(1));
        scene.rect(
            gestureless_proxy.clone(),
            bounds,
            Color::TRANSPARENT,
            Layer(2),
        );
        scene.rect(disabled_proxy.clone(), bounds, Color::TRANSPARENT, Layer(3));
        scene.rect(
            disabled_non_tap_proxy.clone(),
            bounds,
            Color::TRANSPARENT,
            Layer(4),
        );
        scene.rect(
            long_press_target.clone(),
            Bounds2D::from_top_left(
                Place::new(Px(76.0), Px(14.0)),
                Size::new(Px(44.0), Px(44.0)),
            ),
            Color::BLUE,
            Layer(1),
        );
        scene.rect(
            drag_target.clone(),
            Bounds2D::from_top_left(
                Place::new(Px(136.0), Px(14.0)),
                Size::new(Px(44.0), Px(44.0)),
            ),
            Color::BLUE,
            Layer(1),
        );
        let target_action_id = target_action.id;
        let disabled_action_id = disabled_action.id;
        let long_press_action_id = long_press_action.id;
        let disabled_non_tap_action_id = disabled_non_tap_action.id;
        let drag_action_id = drag_action.id;
        let view = Scene2DView::new(scene.finish(fission_game::Tick(0)), 192.0, 72.0)
            .object_actions(
                target,
                SceneObjectActions::new("Lower tap target").on_tap(target_action),
            )
            .object_actions(
                gestureless_proxy.clone(),
                SceneObjectActions::new("Informational proxy")
                    .semantics_identifier("demo.scene.informational"),
            )
            .object_actions(
                disabled_proxy.clone(),
                SceneObjectActions::new("Unavailable proxy")
                    .on_tap(disabled_action)
                    .disabled(true)
                    .semantics_identifier("demo.scene.unavailable"),
            )
            .object_actions(
                long_press_target,
                SceneObjectActions::new("Long-press target").on_long_press(long_press_action),
            )
            .object_actions(
                drag_target,
                SceneObjectActions::new("Drag target").on_drag_start(drag_action),
            )
            .object_actions(
                disabled_non_tap_proxy.clone(),
                SceneObjectActions::new("Unavailable hold proxy")
                    .on_long_press(disabled_non_tap_action)
                    .disabled(true)
                    .semantics_identifier("demo.scene.unavailable-hold"),
            );
        #[derive(Debug, Default)]
        struct DispatchCounts {
            target: usize,
            disabled: usize,
            long_press: usize,
            disabled_non_tap: usize,
            drag: usize,
        }
        impl fission_core::GlobalState for DispatchCounts {}

        let harness =
            TestHarness::new_with_mock_measurer(DispatchCounts::default()).with_root_widget(view);
        let mut driver = TestDriver::new(harness);

        driver
            .pump()
            .expect("overlapping scene objects should render");
        driver
            .harness
            .runtime
            .register_reducer::<DispatchCounts>(target_action_id, |state, _, _| {
                state.target += 1;
                Ok(())
            })
            .expect("target reducer should register");
        driver
            .harness
            .runtime
            .register_reducer::<DispatchCounts>(disabled_action_id, |state, _, _| {
                state.disabled += 1;
                Ok(())
            })
            .expect("disabled reducer should register");
        driver
            .harness
            .runtime
            .register_reducer::<DispatchCounts>(long_press_action_id, |state, _, _| {
                state.long_press += 1;
                Ok(())
            })
            .expect("long-press reducer should register");
        driver
            .harness
            .runtime
            .register_reducer::<DispatchCounts>(disabled_non_tap_action_id, |state, _, _| {
                state.disabled_non_tap += 1;
                Ok(())
            })
            .expect("disabled non-tap reducer should register");
        driver
            .harness
            .runtime
            .register_reducer::<DispatchCounts>(drag_action_id, |state, _, _| {
                state.drag += 1;
                Ok(())
            })
            .expect("drag reducer should register");

        assert_eq!(
            semantic_label_at(&driver.harness, fission_core::LayoutPoint::new(36.0, 36.0))
                .as_deref(),
            Some("Lower tap target"),
            "disabled and gestureless proxies must leave the enabled target reachable"
        );
        assert_eq!(
            semantic_label_at(&driver.harness, fission_core::LayoutPoint::new(98.0, 36.0))
                .as_deref(),
            Some("Long-press target"),
            "an enabled long-press-only object must remain coordinate hittable"
        );
        assert_eq!(
            semantic_label_at(&driver.harness, fission_core::LayoutPoint::new(158.0, 36.0))
                .as_deref(),
            Some("Drag target"),
            "an enabled drag-only object must remain coordinate hittable"
        );

        let long_press_point = fission_core::LayoutPoint::new(98.0, 36.0);
        driver
            .harness
            .send_event(fission_core::InputEvent::Pointer(
                fission_core::PointerEvent::Down {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point: long_press_point,
                    button: fission_core::event::PointerButton::Primary,
                    modifiers: 0,
                },
            ))
            .expect("long press should begin through the runtime input path");
        driver
            .harness
            .tick(500)
            .expect("the deterministic runtime clock should advance");
        driver
            .harness
            .send_event(fission_core::InputEvent::Pointer(
                fission_core::PointerEvent::Up {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point: long_press_point,
                    button: fission_core::event::PointerButton::Primary,
                    modifiers: 0,
                },
            ))
            .expect("long press should end through the runtime input path");
        {
            let counts = driver
                .harness
                .runtime
                .get_app_state::<DispatchCounts>()
                .expect("long-press dispatch state");
            assert_eq!(
                counts.long_press, 1,
                "long press should dispatch exactly once"
            );
            assert_eq!(
                counts.target, 0,
                "long press must not activate another target"
            );
        }

        let drag_start = fission_core::LayoutPoint::new(158.0, 36.0);
        let drag_update = fission_core::LayoutPoint::new(166.0, 36.0);
        driver
            .harness
            .send_event(fission_core::InputEvent::Pointer(
                fission_core::PointerEvent::Down {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point: drag_start,
                    button: fission_core::event::PointerButton::Primary,
                    modifiers: 0,
                },
            ))
            .expect("drag should begin through the runtime input path");
        driver
            .harness
            .send_event(fission_core::InputEvent::Pointer(
                fission_core::PointerEvent::Move {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point: drag_update,
                    modifiers: 0,
                },
            ))
            .expect("drag should cross the runtime movement threshold");
        driver
            .harness
            .send_event(fission_core::InputEvent::Pointer(
                fission_core::PointerEvent::Up {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point: drag_update,
                    button: fission_core::event::PointerButton::Primary,
                    modifiers: 0,
                },
            ))
            .expect("drag should end through the runtime input path");
        assert_eq!(
            driver
                .harness
                .runtime
                .get_app_state::<DispatchCounts>()
                .expect("drag dispatch state")
                .drag,
            1,
            "drag start should dispatch exactly once"
        );

        driver
            .tap_point(36.0, 36.0)
            .expect("coordinate tap should use the runtime input path");
        let counts = driver
            .harness
            .runtime
            .get_app_state::<DispatchCounts>()
            .expect("interaction dispatch state");
        assert_eq!(counts.target, 1, "the lower target should dispatch once");
        assert_eq!(
            counts.disabled, 0,
            "the disabled proxy must never receive the coordinate tap"
        );
        assert_eq!(counts.long_press, 1, "tap must not repeat the long press");
        assert_eq!(counts.drag, 1, "tap must not repeat the drag action");
        assert_eq!(
            counts.disabled_non_tap, 0,
            "the disabled non-tap proxy must never receive pointer input"
        );

        let ir = driver.harness.last_ir.as_ref().expect("pumped IR");
        let informational = ir
            .nodes
            .get(&gestureless_proxy.widget_id())
            .expect("gestureless semantic metadata should remain retained");
        let Op::Semantics(informational) = &informational.op else {
            panic!("gestureless proxy identity should remain semantic");
        };
        assert_eq!(informational.label.as_deref(), Some("Informational proxy"));
        assert_eq!(
            informational.identifier.as_deref(),
            Some("demo.scene.informational")
        );
        assert!(informational.actions.entries.is_empty());

        let unavailable = ir
            .nodes
            .get(&disabled_proxy.widget_id())
            .expect("disabled semantic metadata should remain retained");
        let Op::Semantics(unavailable) = &unavailable.op else {
            panic!("disabled proxy identity should remain semantic");
        };
        assert_eq!(unavailable.label.as_deref(), Some("Unavailable proxy"));
        assert_eq!(
            unavailable.identifier.as_deref(),
            Some("demo.scene.unavailable")
        );
        assert!(unavailable.disabled);
        assert!(unavailable.actions.entries.is_empty());

        let unavailable_hold = ir
            .nodes
            .get(&disabled_non_tap_proxy.widget_id())
            .expect("disabled non-tap semantic metadata should remain retained");
        let Op::Semantics(unavailable_hold) = &unavailable_hold.op else {
            panic!("disabled non-tap proxy identity should remain semantic");
        };
        assert_eq!(
            unavailable_hold.label.as_deref(),
            Some("Unavailable hold proxy")
        );
        assert_eq!(
            unavailable_hold.identifier.as_deref(),
            Some("demo.scene.unavailable-hold")
        );
        assert!(unavailable_hold.disabled);
        assert!(unavailable_hold.actions.entries.is_empty());
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

    fn semantic_label_at<S: fission_core::GlobalState>(
        harness: &TestHarness<S>,
        point: fission_core::LayoutPoint,
    ) -> Option<String> {
        let ir = harness.last_ir.as_ref().expect("pumped IR");
        let layout = harness.last_snapshot.as_ref().expect("pumped layout");
        let mut current = fission_core::hit_test::hit_test(
            ir,
            layout,
            &harness.runtime.runtime_state.scroll,
            point,
        );
        while let Some(id) = current {
            let node = ir.nodes.get(&id)?;
            if let Op::Semantics(semantics) = &node.op {
                return semantics.label.clone();
            }
            current = node.parent;
        }
        None
    }
}
