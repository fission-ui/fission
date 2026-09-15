//! A control drawn smaller than the design system's minimum hit target still
//! answers to pointer input that lands just outside it (Fitts's law), without
//! taking input from anything drawn under the pointer.

use fission_core::{
    event::PointerKind,
    hit_test::{hit_test_with_min_target, HitTargetPolicy},
    LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, Runtime,
};
use fission_ir::{CoreIR, LayoutOp, Op, Semantics, WidgetId};
use fission_layout::LayoutNodeGeometry;

fn control() -> Op {
    Op::Semantics(Semantics {
        focusable: true,
        ..Semantics::default()
    })
}

fn place(snapshot: &mut LayoutSnapshot, id: WidgetId, x: f32, y: f32, w: f32, h: f32) {
    snapshot.nodes.insert(
        id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(x, y, w, h),
            content_size: LayoutSize::new(w, h),
        },
    );
}

fn plain_box() -> Op {
    Op::Layout(LayoutOp::Box {
        width: None,
        height: None,
        min_width: None,
        max_width: None,
        min_height: None,
        max_height: None,
        padding: [0.0; 4],
        flex_grow: 0.0,
        flex_shrink: 1.0,
        aspect_ratio: None,
    })
}

struct Scene {
    ir: CoreIR,
    snapshot: LayoutSnapshot,
    runtime: Runtime,
    small: WidgetId,
    neighbour: WidgetId,
    large: WidgetId,
}

/// A 16x16 control at (100, 100), a second 16x16 control 8px to its right, and
/// a 200x40 control below them.
fn scene() -> Scene {
    let root = WidgetId::derived(0, &[1]);
    let small = WidgetId::derived(0, &[2]);
    let neighbour = WidgetId::derived(0, &[3]);
    let large = WidgetId::derived(0, &[4]);

    let mut ir = CoreIR::new();
    ir.set_root(root);
    ir.add_node(root, plain_box(), vec![small, neighbour, large]);
    ir.add_node(small, control(), vec![]);
    ir.add_node(neighbour, control(), vec![]);
    ir.add_node(large, control(), vec![]);

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
    place(&mut snapshot, root, 0.0, 0.0, 800.0, 600.0);
    place(&mut snapshot, small, 100.0, 100.0, 16.0, 16.0);
    place(&mut snapshot, neighbour, 124.0, 100.0, 16.0, 16.0);
    place(&mut snapshot, large, 100.0, 140.0, 200.0, 40.0);

    Scene {
        ir,
        snapshot,
        runtime: Runtime::default(),
        small,
        neighbour,
        large,
    }
}

fn hit(scene: &Scene, x: f32, y: f32, min_target: f32) -> Option<WidgetId> {
    hit_test_with_min_target(
        &scene.ir,
        &scene.snapshot,
        &scene.runtime.runtime_state.scroll,
        &scene.runtime.runtime_state.viewport,
        LayoutPoint::new(x, y),
        min_target,
    )
}

#[test]
fn a_small_control_answers_just_outside_its_drawn_bounds() {
    let scene = scene();

    // 3px left of the 16px control: outside it, inside its 24px target.
    assert_eq!(hit(&scene, 97.0, 108.0, 24.0), Some(scene.small));
    // Exact hit testing still misses.
    assert_eq!(hit(&scene, 97.0, 108.0, 0.0), None);
    // Beyond the grown area nothing is hit.
    assert_eq!(hit(&scene, 95.0, 108.0, 24.0), None);
}

#[test]
fn touch_input_grows_the_target_further_than_a_mouse() {
    let scene = scene();
    let policy = HitTargetPolicy {
        pointer: 24.0,
        touch: 44.0,
    };

    // 10px above the control: out of reach for a mouse, in reach for touch.
    assert_eq!(
        hit(&scene, 108.0, 90.0, policy.min_size(PointerKind::Mouse)),
        None
    );
    assert_eq!(
        hit(&scene, 108.0, 90.0, policy.min_size(PointerKind::Touch)),
        Some(scene.small)
    );
}

#[test]
fn the_nearest_control_wins_where_grown_targets_overlap() {
    let scene = scene();

    // The gap between the two small controls runs from x=116 to x=124. With a
    // 44px touch target both grown areas cover it, so each side of the middle
    // goes to the control it is closer to.
    assert_eq!(hit(&scene, 118.0, 108.0, 44.0), Some(scene.small));
    assert_eq!(hit(&scene, 122.0, 108.0, 44.0), Some(scene.neighbour));
}

#[test]
fn a_control_under_the_pointer_beats_a_neighbours_grown_target() {
    let scene = scene();

    // Inside the large control, within touch reach of the small one above it.
    assert_eq!(hit(&scene, 108.0, 142.0, 44.0), Some(scene.large));
}

#[test]
fn a_control_already_large_enough_never_grows() {
    let scene = scene();

    // Just below the 200x40 control; it is taller than a pointer target, so
    // it does not reach.
    assert_eq!(hit(&scene, 150.0, 182.0, 24.0), None);
}

#[test]
fn a_disabled_control_does_not_grow() {
    let mut scene = scene();
    if let Some(node) = scene.ir.nodes.get_mut(&scene.small) {
        node.op = Op::Semantics(Semantics {
            focusable: true,
            disabled: true,
            ..Semantics::default()
        });
    }

    assert_eq!(hit(&scene, 97.0, 108.0, 24.0), None);
}

#[test]
fn the_runtime_uses_the_default_design_system_targets() {
    let runtime = Runtime::default();
    let policy = runtime.hit_target_policy();

    assert_eq!(policy.min_size(PointerKind::Mouse), 24.0);
    assert_eq!(policy.min_size(PointerKind::Touch), 48.0);
}
