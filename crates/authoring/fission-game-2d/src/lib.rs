//! Retained Fission widgets for renderer-independent [`Scene2DIR`] output.
//!
//! The game runtime remains headless. This crate is the graphical adapter that
//! turns the same validated scene into ordinary Fission widgets, preserving
//! layout, accessibility, hit testing, and shell portability.

use std::collections::BTreeMap;

use fission_core::ui::widgets::Transform;
use fission_core::ui::{
    Composite, Container, Image, Positioned, Pressable, PressableRole, Text, Widget, ZStack,
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
    taps: BTreeMap<SceneNodeId, SceneTapAction>,
}

impl Scene2DView {
    pub fn new(scene: Scene2DIR, width: f32, height: f32) -> Self {
        Self {
            scene,
            width: finite_non_negative(width),
            height: finite_non_negative(height),
            taps: BTreeMap::new(),
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
        self.taps.insert(
            id,
            SceneTapAction {
                label: label.into(),
                action,
            },
        );
        self
    }
}

impl From<Scene2DView> for Widget {
    fn from(view: Scene2DView) -> Self {
        let mut children = Vec::new();
        for command in view.scene.commands {
            append_command(&mut children, command, &view.taps);
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
    taps: &BTreeMap<SceneNodeId, SceneTapAction>,
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
                with_interaction(id, with_opacity(visual, opacity), taps),
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
            taps,
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
                with_interaction(id, visual, taps),
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
                    taps,
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
    taps: &BTreeMap<SceneNodeId, SceneTapAction>,
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
    positioned(id.clone(), bounds, with_interaction(id, visual, taps))
}

fn with_interaction(
    id: SceneNodeId,
    visual: Widget,
    taps: &BTreeMap<SceneNodeId, SceneTapAction>,
) -> Widget {
    let retained_id = id.widget_id();
    if let Some(tap) = taps.get(&id) {
        Pressable {
            id: Some(retained_id),
            child: visual,
            on_press: Some(tap.action.clone()),
            label: Some(tap.label.clone()),
            semantics_identifier: Some(format!("game.scene.{}", retained_id.as_u128())),
            role: PressableRole::Button,
            ..Default::default()
        }
        .into()
    } else {
        Container {
            id: Some(retained_id),
            child: Some(visual),
            ..Default::default()
        }
        .into()
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
    use fission_ir::op::{Color, Op};

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
