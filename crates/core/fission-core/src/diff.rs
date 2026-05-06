use fission_ir::{CoreIR, CoreNode, NodeId, Op, PaintOp};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct FrameDiff {
    pub dirty_layout: HashSet<NodeId>,
    pub dirty_paint: HashSet<NodeId>,
    pub dirty_composite: HashSet<NodeId>,
}

pub fn diff_ir(prev: &CoreIR, next: &CoreIR) -> FrameDiff {
    let mut diff = FrameDiff::default();

    if prev.root != next.root {
        let all_nodes: HashSet<NodeId> = next.nodes.keys().copied().collect();
        diff.dirty_layout = all_nodes.clone();
        diff.dirty_paint = all_nodes.clone();
        diff.dirty_composite = all_nodes;
        return diff;
    }

    for (id, next_node) in &next.nodes {
        match prev.nodes.get(id) {
            None => {
                diff.dirty_layout.insert(*id);
                diff.dirty_paint.insert(*id);
                diff.dirty_composite.insert(*id);
            }
            Some(prev_node) => {
                if node_requires_layout(prev_node, next_node) {
                    diff.dirty_layout.insert(*id);
                    continue;
                }

                if prev_node.composite != next_node.composite {
                    diff.dirty_composite.insert(*id);
                }

                if node_requires_paint(prev_node, next_node) {
                    diff.dirty_paint.insert(*id);
                }
            }
        }
    }

    diff
}

fn node_requires_layout(prev: &CoreNode, next: &CoreNode) -> bool {
    if prev.children != next.children || prev.parent != next.parent {
        return true;
    }

    match (&prev.op, &next.op) {
        (Op::Layout(prev_op), Op::Layout(next_op)) => prev_op != next_op,
        (Op::Structural(prev_op), Op::Structural(next_op)) => prev_op != next_op,
        (Op::Paint(prev_op), Op::Paint(next_op)) => paint_change_requires_layout(prev_op, next_op),
        (Op::Semantics(_), Op::Semantics(_)) => false,
        _ => true,
    }
}

fn node_requires_paint(prev: &CoreNode, next: &CoreNode) -> bool {
    match (&prev.op, &next.op) {
        (Op::Paint(prev_op), Op::Paint(next_op)) => prev_op != next_op,
        (Op::Semantics(_), Op::Semantics(_)) => false,
        _ => false,
    }
}

fn paint_change_requires_layout(prev: &PaintOp, next: &PaintOp) -> bool {
    match (prev, next) {
        (PaintOp::DrawText { .. }, PaintOp::DrawText { .. }) => prev != next,
        (PaintOp::DrawRichText { .. }, PaintOp::DrawRichText { .. }) => prev != next,
        (PaintOp::DrawText { .. }, _) | (_, PaintOp::DrawText { .. }) => true,
        (PaintOp::DrawRichText { .. }, _) | (_, PaintOp::DrawRichText { .. }) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::diff_ir;
    use fission_ir::op::Fill;
    use fission_ir::{CompositeScalar, CompositeStyle, CoreIR, LayoutOp, NodeId, Op, PaintOp};

    fn rect_ir(id_seed: u128, color: (u8, u8, u8, u8)) -> CoreIR {
        let root = NodeId::derived(id_seed, &[0]);
        let paint = NodeId::derived(id_seed, &[1]);
        let mut ir = CoreIR::new();
        ir.add_node(
            paint,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(fission_ir::op::Color {
                    r: color.0,
                    g: color.1,
                    b: color.2,
                    a: color.3,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node(
            root,
            Op::Layout(LayoutOp::Box {
                width: Some(10.0),
                height: Some(10.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![paint],
        );
        ir.set_root(root);
        for node in ir.nodes.values_mut() {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            node.op.hash(&mut hasher);
            node.composite.hash(&mut hasher);
            node.children.hash(&mut hasher);
            node.parent.hash(&mut hasher);
            node.hash = hasher.finish();
        }
        ir
    }

    #[test]
    fn paint_only_changes_do_not_force_layout() {
        let prev = rect_ir(1, (255, 0, 0, 255));
        let next = rect_ir(1, (255, 0, 0, 128));
        let diff = diff_ir(&prev, &next);
        assert!(
            diff.dirty_layout.is_empty(),
            "paint-only changes should not invalidate layout"
        );
        assert_eq!(diff.dirty_paint.len(), 1);
    }

    #[test]
    fn layout_changes_still_force_layout() {
        let prev = rect_ir(2, (255, 0, 0, 255));
        let mut next = rect_ir(2, (255, 0, 0, 255));
        let root = next.root.expect("root");
        if let Some(node) = next.nodes.get_mut(&root) {
            node.op = Op::Layout(LayoutOp::Box {
                width: Some(20.0),
                height: Some(10.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            });
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            node.op.hash(&mut hasher);
            node.composite.hash(&mut hasher);
            node.children.hash(&mut hasher);
            node.parent.hash(&mut hasher);
            node.hash = hasher.finish();
        }
        let diff = diff_ir(&prev, &next);
        assert!(diff.dirty_layout.contains(&root));
    }

    #[test]
    fn text_paint_changes_force_layout() {
        let root = NodeId::derived(3, &[0]);
        let text = NodeId::derived(3, &[1]);
        let mut prev = CoreIR::new();
        prev.add_node(
            text,
            Op::Paint(PaintOp::DrawText {
                text: "a".into(),
                size: 12.0,
                color: fission_ir::op::Color::BLACK,
                underline: false,
                wrap: true,
                caret_index: None,
            }),
            vec![],
        );
        prev.add_node(
            root,
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
            }),
            vec![text],
        );
        prev.set_root(root);
        let mut next = prev.clone();
        if let Some(node) = next.nodes.get_mut(&text) {
            node.op = Op::Paint(PaintOp::DrawText {
                text: "much wider".into(),
                size: 12.0,
                color: fission_ir::op::Color::BLACK,
                underline: false,
                wrap: true,
                caret_index: None,
            });
        }
        let diff = diff_ir(&prev, &next);
        assert!(diff.dirty_layout.contains(&text));
    }

    #[test]
    fn composite_changes_do_not_force_layout() {
        let prev = rect_ir(4, (255, 0, 0, 255));
        let mut next = rect_ir(4, (255, 0, 0, 255));
        let root = next.root.expect("root");
        if let Some(node) = next.nodes.get_mut(&root) {
            node.composite = CompositeStyle {
                opacity: Some(CompositeScalar::new(0.5)),
                ..Default::default()
            };
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            node.op.hash(&mut hasher);
            node.composite.hash(&mut hasher);
            node.children.hash(&mut hasher);
            node.parent.hash(&mut hasher);
            node.hash = hasher.finish();
        }
        let diff = diff_ir(&prev, &next);
        assert!(!diff.dirty_layout.contains(&root));
        assert!(diff.dirty_composite.contains(&root));
    }
}
