use anyhow::Result;
use fission_core::{Runtime, TextEditSource};
use fission_ir::{
    op::{ResponsiveCondition, ResponsiveQuery},
    CoreIR, LayoutOp, Op, Role, Semantics, WidgetId,
};
use fission_layout::{LayoutNodeGeometry, LayoutRect, LayoutSize, LayoutSnapshot};

fn box_op() -> Op {
    Op::Layout(LayoutOp::Box {
        width: None,
        height: None,
        min_width: None,
        max_width: None,
        min_height: None,
        max_height: None,
        padding: [0.0; 4],
        flex_grow: 0.0,
        flex_shrink: 0.0,
        aspect_ratio: None,
    })
}

fn responsive_editor_tree() -> (CoreIR, WidgetId, WidgetId, WidgetId) {
    let root = WidgetId::explicit("root");
    let compact_branch = WidgetId::explicit("compact-branch");
    let compact_input = WidgetId::explicit("compact-input");
    let expanded_branch = WidgetId::explicit("expanded-branch");
    let expanded_input = WidgetId::explicit("expanded-input");

    let input = || {
        Op::Semantics(Semantics {
            role: Role::TextInput,
            identifier: Some("document.editor".into()),
            focusable: true,
            ..Semantics::default()
        })
    };
    let mut ir = CoreIR::new();
    ir.add_node(compact_input, input(), vec![]);
    ir.add_node(compact_branch, box_op(), vec![compact_input]);
    ir.add_node(expanded_input, input(), vec![]);
    ir.add_node(expanded_branch, box_op(), vec![expanded_input]);
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Responsive {
            query: ResponsiveQuery::Viewport,
            cases: vec![ResponsiveCondition {
                min_width: None,
                max_width: Some(600.0),
            }],
        }),
        vec![compact_branch, expanded_branch],
    );
    ir.set_root(root);
    (ir, root, compact_input, expanded_input)
}

fn responsive_layout(
    root: WidgetId,
    active_branch: WidgetId,
    active_input: WidgetId,
) -> LayoutSnapshot {
    let mut layout = LayoutSnapshot::new(LayoutSize::new(1_100.0, 700.0));
    for (id, rect) in [
        (root, LayoutRect::new(0.0, 0.0, 1_100.0, 700.0)),
        (active_branch, LayoutRect::new(0.0, 0.0, 1_100.0, 700.0)),
        (active_input, LayoutRect::new(20.0, 20.0, 400.0, 80.0)),
    ] {
        layout.nodes.insert(
            id,
            LayoutNodeGeometry {
                rect,
                content_size: LayoutSize::new(rect.width(), rect.height()),
            },
        );
    }
    layout
}

fn rebuilt_editor_tree(input_id: WidgetId) -> CoreIR {
    let root_label = format!("{}-screen", input_id);
    let root = WidgetId::explicit(&root_label);
    let mut ir = CoreIR::new();
    ir.add_node(
        input_id,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            identifier: Some("document.editor".into()),
            focusable: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(root, box_op(), vec![input_id]);
    ir.set_root(root);
    ir
}

#[test]
fn focus_moves_between_responsive_inputs_with_the_same_semantic_identity() -> Result<()> {
    let (ir, root, compact_input, expanded_input) = responsive_editor_tree();
    let expanded_branch = ir.nodes[&expanded_input].parent.unwrap();
    let compact_branch = ir.nodes[&compact_input].parent.unwrap();
    let expanded_layout = responsive_layout(root, expanded_branch, expanded_input);
    let compact_layout = responsive_layout(root, compact_branch, compact_input);

    assert!(fission_core::hit_test::is_in_active_responsive_branch(
        &ir,
        &expanded_layout,
        expanded_input
    ));
    assert!(!fission_core::hit_test::is_in_active_responsive_branch(
        &ir,
        &expanded_layout,
        compact_input
    ));
    assert!(fission_core::hit_test::is_in_active_responsive_branch(
        &ir,
        &compact_layout,
        compact_input
    ));

    let mut runtime = Runtime::default();
    runtime.set_focused_widget(&ir, Some(expanded_input), TextEditSource::Programmatic)?;
    runtime.reconcile_focus_with_layout(&ir, &expanded_layout)?;
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(expanded_input)
    );

    runtime.reconcile_focus_with_layout(&ir, &compact_layout)?;
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(compact_input)
    );

    runtime.reconcile_focus_with_layout(&ir, &expanded_layout)?;
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(expanded_input)
    );
    Ok(())
}

#[test]
fn focus_survives_a_responsive_rebuild_that_replaces_the_focused_widget_id() -> Result<()> {
    let compact_input = WidgetId::explicit("compact-editor");
    let expanded_input = WidgetId::explicit("expanded-editor");
    let compact_ir = rebuilt_editor_tree(compact_input);
    let expanded_ir = rebuilt_editor_tree(expanded_input);
    let mut runtime = Runtime::default();
    runtime.set_focused_widget(
        &expanded_ir,
        Some(expanded_input),
        TextEditSource::Programmatic,
    )?;
    runtime.reconcile_focus(&expanded_ir)?;

    runtime.reconcile_focus(&compact_ir)?;
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(compact_input)
    );

    runtime.reconcile_focus(&expanded_ir)?;
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(expanded_input)
    );
    Ok(())
}
