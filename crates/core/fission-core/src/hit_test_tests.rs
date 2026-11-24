use super::{
    active_responsive_child, canvas_target_hit, focus_barriers_in_tree_order,
    get_all_focusable_nodes, hit_test, is_enabled_focus_node, is_interaction_inert,
    preferred_focus_node_in_scope, topmost_semantics_action,
};
use crate::env::ScrollStateMap;
use fission_ir::op::ResponsiveQuery;
use fission_ir::{
    ActionEntry, ActionSet, ActionTrigger, CanvasSelectionPolicy, CanvasTarget, CanvasTargetKind,
    CoreIR, LayoutOp, Op, Role, Semantics, StructuralOp, WidgetId,
};
use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};

fn edge(points: Vec<[f32; 2]>, cubic: bool) -> CanvasTarget {
    CanvasTarget {
        canvas_id: 1,
        kind: CanvasTargetKind::Edge {
            edge_id: 2,
            points,
            cubic,
            hit_tolerance: 4.0,
        },
        selection_policy: CanvasSelectionPolicy::Single,
        snap_spacing: None,
        snap_threshold: 0.0,
    }
}

#[test]
fn active_responsive_child_search_is_stack_safe_for_deep_branches() {
    let mut ir = CoreIR::default();
    let mut inactive_branch = WidgetId::explicit("deep-branch-leaf");
    ir.add_node(
        inactive_branch,
        Op::Structural(StructuralOp::Group { stable_hash: 0 }),
        Vec::new(),
    );
    for depth in 1..=20_000_u64 {
        let parent = WidgetId::explicit(&format!("deep-branch-{depth}"));
        ir.add_node(
            parent,
            Op::Structural(StructuralOp::Group { stable_hash: depth }),
            vec![inactive_branch],
        );
        inactive_branch = parent;
    }

    let active_branch = WidgetId::explicit("active-branch");
    ir.add_node(
        active_branch,
        Op::Structural(StructuralOp::Group {
            stable_hash: 20_001,
        }),
        Vec::new(),
    );
    let responsive = WidgetId::explicit("responsive-root");
    ir.add_node(
        responsive,
        Op::Layout(LayoutOp::Responsive {
            query: ResponsiveQuery::Viewport,
            cases: Vec::new(),
        }),
        vec![inactive_branch, active_branch],
    );
    ir.set_root(responsive);

    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    layout.nodes.insert(
        active_branch,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 100.0),
        },
    );

    assert_eq!(
        active_responsive_child(&ir, &layout, responsive),
        Some(active_branch)
    );
}

#[test]
fn edge_hit_testing_uses_stroke_geometry_instead_of_its_bounding_box() {
    let straight = edge(vec![[10.0, 10.0], [90.0, 90.0]], false);
    assert!(canvas_target_hit(&straight, LayoutPoint::new(50.0, 52.0)));
    assert!(!canvas_target_hit(&straight, LayoutPoint::new(10.0, 90.0)));

    let polyline = edge(
        vec![[10.0, 10.0], [40.0, 10.0], [40.0, 80.0], [90.0, 80.0]],
        false,
    );
    assert!(canvas_target_hit(&polyline, LayoutPoint::new(75.0, 82.0)));
    assert!(!canvas_target_hit(&polyline, LayoutPoint::new(75.0, 20.0)));

    let cubic = edge(
        vec![[0.0, 50.0], [25.0, 0.0], [75.0, 100.0], [100.0, 50.0]],
        true,
    );
    assert!(canvas_target_hit(&cubic, LayoutPoint::new(50.0, 50.0)));
    assert!(!canvas_target_hit(&cubic, LayoutPoint::new(50.0, 5.0)));
}

#[test]
fn pointer_transparent_groups_pass_hits_to_underlying_siblings() {
    let root = WidgetId::explicit("root");
    let background = WidgetId::explicit("background-button");
    let wrapper = WidgetId::explicit("pointer-transparent");
    let overlay_child = WidgetId::explicit("interactive-overlay-child");
    let mut ir = CoreIR::default();
    ir.add_node(
        background,
        Op::Semantics(Semantics {
            role: Role::Button,
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        overlay_child,
        Op::Semantics(Semantics {
            role: Role::Button,
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        wrapper,
        Op::Structural(StructuralOp::PointerTransparent { stable_hash: 1 }),
        vec![overlay_child],
    );
    ir.add_node(
        root,
        Op::Structural(StructuralOp::Group { stable_hash: 2 }),
        vec![background, wrapper],
    );
    ir.set_root(root);
    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    for id in [root, background, wrapper, overlay_child] {
        layout.nodes.insert(
            id,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
                content_size: LayoutSize::new(100.0, 100.0),
            },
        );
    }

    assert_eq!(
        hit_test(
            &ir,
            &layout,
            &ScrollStateMap::default(),
            LayoutPoint::new(50.0, 50.0),
        ),
        Some(background)
    );
}

#[test]
fn interaction_inert_groups_are_visual_only_for_pointer_focus_and_barriers() {
    let root = WidgetId::explicit("root");
    let background = WidgetId::explicit("background-button");
    let wrapper = WidgetId::explicit("interaction-inert");
    let barrier = WidgetId::explicit("exiting-focus-barrier");
    let overlay_child = WidgetId::explicit("exiting-button");
    let mut ir = CoreIR::default();
    ir.add_node(
        background,
        Op::Semantics(Semantics {
            role: Role::Button,
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        overlay_child,
        Op::Semantics(Semantics {
            role: Role::Button,
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        barrier,
        Op::Semantics(Semantics {
            is_focus_scope: true,
            is_focus_barrier: true,
            ..Semantics::default()
        }),
        vec![overlay_child],
    );
    ir.add_node(
        wrapper,
        Op::Structural(StructuralOp::InteractionInert { stable_hash: 1 }),
        vec![barrier],
    );
    ir.add_node(
        root,
        Op::Structural(StructuralOp::Group { stable_hash: 2 }),
        vec![background, wrapper],
    );
    ir.set_root(root);
    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
    for id in [root, background, wrapper, barrier, overlay_child] {
        layout.nodes.insert(
            id,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
                content_size: LayoutSize::new(100.0, 100.0),
            },
        );
    }

    assert_eq!(
        hit_test(
            &ir,
            &layout,
            &ScrollStateMap::default(),
            LayoutPoint::new(50.0, 50.0),
        ),
        Some(background)
    );
    assert_eq!(get_all_focusable_nodes(&ir), vec![background]);
    assert!(focus_barriers_in_tree_order(&ir).is_empty());
    assert!(!is_enabled_focus_node(&ir, overlay_child));
    assert!(is_interaction_inert(&ir, overlay_child));
}

#[test]
fn programmatic_focus_targets_can_opt_out_of_sequential_traversal() {
    let root = WidgetId::explicit("root");
    let regular = WidgetId::explicit("regular");
    let composite_child = WidgetId::explicit("composite-child");
    let mut ir = CoreIR::default();
    ir.add_node(
        regular,
        Op::Semantics(Semantics {
            role: Role::Button,
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        composite_child,
        Op::Semantics(Semantics {
            role: Role::Option,
            focusable: true,
            sequential_focusable: false,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Structural(StructuralOp::Group { stable_hash: 1 }),
        vec![regular, composite_child],
    );
    ir.set_root(root);

    assert_eq!(get_all_focusable_nodes(&ir), vec![regular]);
    assert!(is_enabled_focus_node(&ir, composite_child));
}

#[test]
fn focus_scope_entry_prefers_sequential_then_programmatic_only_fallback() {
    let scope = WidgetId::explicit("scope");
    let surface = WidgetId::explicit("programmatic-surface");
    let control = WidgetId::explicit("sequential-control");
    let build_ir = |with_control| {
        let mut ir = CoreIR::default();
        ir.add_node(
            surface,
            Op::Semantics(Semantics {
                role: Role::Dialog,
                focusable: true,
                sequential_focusable: false,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        let mut children = vec![surface];
        if with_control {
            ir.add_node(
                control,
                Op::Semantics(Semantics {
                    role: Role::Button,
                    focusable: true,
                    sequential_focusable: true,
                    ..Semantics::default()
                }),
                Vec::new(),
            );
            children.push(control);
        }
        ir.add_node(
            scope,
            Op::Semantics(Semantics {
                is_focus_scope: true,
                ..Semantics::default()
            }),
            children,
        );
        ir.set_root(scope);
        ir
    };

    assert_eq!(
        preferred_focus_node_in_scope(&build_ir(true), scope),
        Some(control),
        "an earlier programmatic-only surface must not displace a sequential control"
    );
    assert_eq!(
        preferred_focus_node_in_scope(&build_ir(false), scope),
        Some(surface),
        "a scope without sequential controls must retain a programmatic entry target"
    );
}

#[test]
fn topmost_action_ignores_exiting_interaction_inert_overlays() {
    let root = WidgetId::explicit("root");
    let active = WidgetId::explicit("active-overlay");
    let inert = WidgetId::explicit("inert-overlay");
    let inert_action = WidgetId::explicit("inert-action");
    let action = |action_id| ActionSet {
        entries: vec![ActionEntry {
            trigger: ActionTrigger::Dismiss,
            action_id,
            payload_data: Some(Vec::new()),
        }],
    };
    let mut ir = CoreIR::default();
    ir.add_node(
        active,
        Op::Semantics(Semantics {
            actions: action(7),
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        inert_action,
        Op::Semantics(Semantics {
            actions: action(9),
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        inert,
        Op::Structural(StructuralOp::InteractionInert { stable_hash: 2 }),
        vec![inert_action],
    );
    ir.add_node(
        root,
        Op::Structural(StructuralOp::Group { stable_hash: 1 }),
        vec![active, inert],
    );
    ir.set_root(root);

    let (target, entry) =
        topmost_semantics_action(&ir, ActionTrigger::Dismiss).expect("active dismissal action");
    assert_eq!(target, active);
    assert_eq!(entry.action_id, 7);
}
