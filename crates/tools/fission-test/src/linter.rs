use fission_ir::{CoreIR, LayoutOp, NodeId, Op};
use fission_layout::{LayoutRect, LayoutSnapshot};

#[derive(Debug)]
pub enum LayoutViolation {
    Overflow {
        parent: NodeId,
        child: NodeId,
        parent_rect: LayoutRect,
        child_rect: LayoutRect,
    },
    ZeroSizeInteractive {
        node: NodeId,
        rect: LayoutRect,
        role: String,
    },
    // Add more violations as needed
}

pub struct LayoutLinter<'a> {
    ir: &'a CoreIR,
    snapshot: &'a LayoutSnapshot,
}

impl<'a> LayoutLinter<'a> {
    pub fn new(ir: &'a CoreIR, snapshot: &'a LayoutSnapshot) -> Self {
        Self { ir, snapshot }
    }

    pub fn check(&self) -> Vec<LayoutViolation> {
        let mut violations = Vec::new();
        if let Some(root) = self.ir.root {
            self.check_recursive(root, &mut violations);
        }
        violations
    }

    fn check_recursive(&self, node_id: NodeId, violations: &mut Vec<LayoutViolation>) {
        let node = self.ir.nodes.get(&node_id).expect("Node missing in IR");
        let geom = self.snapshot.get_node_geometry(node_id);

        if let Some(geom) = geom {
            // Check Interactive Visibility
            if let Op::Semantics(s) = &node.op {
                if s.focusable || !s.actions.entries.is_empty() {
                    if geom.rect.width() < 1.0 || geom.rect.height() < 1.0 {
                        violations.push(LayoutViolation::ZeroSizeInteractive {
                            node: node_id,
                            rect: geom.rect,
                            role: format!("{:?}", s.role),
                        });
                    }
                }
            }

            // Check Containment (if not Scroll or Overflow allowed)
            let allows_overflow = matches!(node.op, Op::Layout(LayoutOp::Scroll { .. }));

            // Note: Absolute children (Positioned, AbsoluteFill) are relative to nearest positioned ancestor,
            // not necessarily direct parent. For simple checking, we might skip them or check against the appropriate ancestor.
            // For now, let's check direct flow children.

            for child_id in &node.children {
                if let Some(_child_geom) = self.snapshot.get_node_geometry(*child_id) {
                    if let Some(child_node) = self.ir.nodes.get(child_id) {
                        let is_absolute = matches!(
                            child_node.op,
                            Op::Layout(LayoutOp::Positioned { .. })
                                | Op::Layout(LayoutOp::AbsoluteFill)
                        );

                        if !allows_overflow && !is_absolute {
                            // Check if child is roughly inside parent (allow small float error)
                            // A child can be smaller than parent, but shouldn't exceed bounds unless allowed.
                            // Overflow is visible by default in our layout model.
                            // But "Toast border smaller than content" is an overflow issue we want to catch.
                            // Let's flag if child is strictly larger than parent in dimensions?
                            // Or if child rect is not contained in parent rect?
                            // Many UIs intentionally overflow (shadows, badges).
                            // Let's focus on "Content rect > Parent Border Rect" for Containers.
                        }
                    }
                    self.check_recursive(*child_id, violations);
                }
            }
        }
    }
}
