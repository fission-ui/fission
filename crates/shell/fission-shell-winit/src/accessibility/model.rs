use std::collections::HashSet;

use fission_core::Runtime;
use fission_ir::semantics::{ActionTrigger, Role, TextInputAction, TextInputType};
use fission_ir::{CoreIR, Op, PaintOp, Semantics, WidgetId};
use fission_layout::{LayoutRect, LayoutSize, LayoutSnapshot};

use crate::driver_support::{clipped_visible_rect_for_node, visual_rect_for_node};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SemanticSnapshot {
    pub(crate) viewport: LayoutSize,
    pub(crate) roots: Vec<WidgetId>,
    pub(crate) nodes: Vec<SemanticNode>,
    pub(crate) focused: Option<WidgetId>,
}

impl SemanticSnapshot {
    pub(crate) fn build(ir: &CoreIR, layout: &LayoutSnapshot, runtime: &Runtime) -> Self {
        let mut builder = SnapshotBuilder {
            ir,
            layout,
            runtime,
            nodes: Vec::new(),
            seen: HashSet::new(),
        };
        let roots = ir
            .root
            .map(|root| builder.collect_subtree(root, None))
            .unwrap_or_default();
        Self {
            viewport: layout.viewport_size,
            roots,
            nodes: builder.nodes,
            focused: runtime.runtime_state.interaction.focused,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SemanticNode {
    pub(crate) id: WidgetId,
    pub(crate) parent: Option<WidgetId>,
    pub(crate) children: Vec<WidgetId>,
    pub(crate) bounds: Option<LayoutRect>,
    pub(crate) visible_bounds: Option<LayoutRect>,
    pub(crate) role: Role,
    pub(crate) label: Option<String>,
    pub(crate) identifier: Option<String>,
    pub(crate) value: Option<String>,
    pub(crate) focusable: bool,
    pub(crate) focused: bool,
    pub(crate) disabled: bool,
    pub(crate) read_only: bool,
    pub(crate) checked: Option<bool>,
    pub(crate) min_value: Option<f32>,
    pub(crate) max_value: Option<f32>,
    pub(crate) current_value: Option<f32>,
    pub(crate) selection: Option<(usize, usize)>,
    pub(crate) multiline: bool,
    pub(crate) masked: bool,
    pub(crate) text_input_type: TextInputType,
    pub(crate) text_input_action: TextInputAction,
    pub(crate) scrollable_x: bool,
    pub(crate) scrollable_y: bool,
    pub(crate) actions: Vec<ActionTrigger>,
}

impl SemanticNode {
    pub(crate) fn is_text_control(&self) -> bool {
        self.role == Role::TextInput
    }

    pub(crate) fn supports(&self, trigger: ActionTrigger) -> bool {
        self.actions.contains(&trigger)
    }
}

struct SnapshotBuilder<'a> {
    ir: &'a CoreIR,
    layout: &'a LayoutSnapshot,
    runtime: &'a Runtime,
    nodes: Vec<SemanticNode>,
    seen: HashSet<WidgetId>,
}

impl SnapshotBuilder<'_> {
    fn collect_subtree(
        &mut self,
        id: WidgetId,
        semantic_parent: Option<WidgetId>,
    ) -> Vec<WidgetId> {
        if !self.seen.insert(id) {
            return Vec::new();
        }
        let Some(core_node) = self.ir.nodes.get(&id) else {
            return Vec::new();
        };
        match &core_node.op {
            Op::Semantics(semantics) if include_semantics(semantics) => {
                // Keep retained DOM insertion order aligned with semantic tree
                // order. Browser tab navigation follows document order, not
                // `aria-owns`, so inserting children before their parent would
                // make nested focus targets traverse backwards.
                let node_index = self.nodes.len();
                self.nodes
                    .push(self.semantic_node(id, semantic_parent, Vec::new(), semantics));
                let children = core_node
                    .children
                    .iter()
                    .flat_map(|child| self.collect_subtree(*child, Some(id)))
                    .collect::<Vec<_>>();
                self.nodes[node_index].children = children;
                vec![id]
            }
            Op::Paint(PaintOp::DrawText { text, .. }) if !text.is_empty() => {
                self.nodes
                    .push(self.text_node(id, semantic_parent, text.clone()));
                vec![id]
            }
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
                if text.is_empty() {
                    Vec::new()
                } else {
                    self.nodes.push(self.text_node(id, semantic_parent, text));
                    vec![id]
                }
            }
            _ => core_node
                .children
                .iter()
                .flat_map(|child| self.collect_subtree(*child, semantic_parent))
                .collect(),
        }
    }

    fn semantic_node(
        &self,
        id: WidgetId,
        parent: Option<WidgetId>,
        children: Vec<WidgetId>,
        semantics: &Semantics,
    ) -> SemanticNode {
        let runtime_edit = self.runtime.runtime_state.text_edit.get(id);
        let focused = self.runtime.runtime_state.interaction.focused == Some(id);
        let value = if semantics.role == Role::TextInput {
            runtime_edit
                .filter(|state| (focused && state.pending_model_sync) || semantics.value.is_none())
                .map(|state| state.committed_text())
                .or_else(|| semantics.value.clone())
                .or_else(|| runtime_edit.map(|state| state.committed_text()))
        } else {
            semantics.value.clone()
        };
        let selection = if semantics.role == Role::TextInput && focused {
            runtime_edit
                .map(|state| (state.anchor, state.caret))
                .or(semantics.text_selection)
        } else {
            semantics.text_selection
        };
        SemanticNode {
            id,
            parent,
            children,
            bounds: visual_rect_for_node(
                self.ir,
                self.layout,
                &self.runtime.runtime_state.scroll,
                id,
            ),
            visible_bounds: clipped_visible_rect_for_node(
                self.ir,
                self.layout,
                &self.runtime.runtime_state.scroll,
                id,
            ),
            role: semantics.role,
            label: semantics
                .label
                .clone()
                .or_else(|| collect_descendant_text(self.ir, id)),
            identifier: semantics.identifier.clone(),
            value,
            focusable: semantics.focusable,
            focused,
            disabled: semantics.disabled,
            read_only: semantics.read_only,
            checked: semantics.checked,
            min_value: semantics.min_value,
            max_value: semantics.max_value,
            current_value: semantics.current_value,
            selection,
            multiline: semantics.multiline || semantics.text_input_type == TextInputType::Multiline,
            masked: semantics.masked,
            text_input_type: semantics.text_input_type,
            text_input_action: semantics.text_input_action,
            scrollable_x: semantics.scrollable_x,
            scrollable_y: semantics.scrollable_y,
            actions: semantics
                .actions
                .entries
                .iter()
                .map(|entry| entry.trigger)
                .collect(),
        }
    }

    fn text_node(&self, id: WidgetId, parent: Option<WidgetId>, text: String) -> SemanticNode {
        SemanticNode {
            id,
            parent,
            children: Vec::new(),
            bounds: visual_rect_for_node(
                self.ir,
                self.layout,
                &self.runtime.runtime_state.scroll,
                id,
            ),
            visible_bounds: clipped_visible_rect_for_node(
                self.ir,
                self.layout,
                &self.runtime.runtime_state.scroll,
                id,
            ),
            role: Role::Text,
            label: None,
            identifier: None,
            value: Some(text),
            focusable: false,
            focused: false,
            disabled: false,
            read_only: true,
            checked: None,
            min_value: None,
            max_value: None,
            current_value: None,
            selection: None,
            multiline: false,
            masked: false,
            text_input_type: TextInputType::Text,
            text_input_action: TextInputAction::Done,
            scrollable_x: false,
            scrollable_y: false,
            actions: Vec::new(),
        }
    }
}

fn include_semantics(semantics: &Semantics) -> bool {
    semantics.role != Role::Generic
        || semantics.label.is_some()
        || semantics.identifier.is_some()
        || semantics.value.is_some()
        || semantics.focusable
        || semantics.checked.is_some()
        || semantics.current_value.is_some()
        || semantics.scrollable_x
        || semantics.scrollable_y
        || !semantics.actions.entries.is_empty()
}

fn collect_descendant_text(ir: &CoreIR, id: WidgetId) -> Option<String> {
    let mut text = String::new();
    collect_descendant_text_inner(ir, id, &mut text);
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn collect_descendant_text_inner(ir: &CoreIR, id: WidgetId, out: &mut String) {
    let Some(node) = ir.nodes.get(&id) else {
        return;
    };
    match &node.op {
        Op::Paint(PaintOp::DrawText { text, .. }) => push_text(out, text),
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
            let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
            push_text(out, &text);
        }
        _ => {
            for child in &node.children {
                collect_descendant_text_inner(ir, *child, out);
            }
        }
    }
}

fn push_text(out: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::ScrollStateMap;
    use fission_ir::{CoreNode, FlexDirection, LayoutOp};
    use fission_layout::{LayoutNodeGeometry, LayoutPoint};

    fn add_node(ir: &mut CoreIR, id: WidgetId, op: Op, children: Vec<WidgetId>) {
        ir.nodes.insert(
            id,
            CoreNode {
                id,
                op,
                composite: Default::default(),
                children: children.clone(),
                parent: None,
                hash: 0,
            },
        );
        for child in children {
            ir.nodes.get_mut(&child).unwrap().parent = Some(id);
        }
    }

    fn geometry(rect: LayoutRect, content_size: LayoutSize) -> LayoutNodeGeometry {
        LayoutNodeGeometry { rect, content_size }
    }

    #[test]
    fn snapshot_keeps_widget_identity_and_runtime_text_state() {
        let input = WidgetId::from_u128(7);
        let mut ir = CoreIR::new();
        add_node(
            &mut ir,
            input,
            Op::Semantics(Semantics {
                role: Role::TextInput,
                label: Some("Search".into()),
                value: Some("model".into()),
                text_selection: Some((0, 0)),
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.root = Some(input);
        let mut layout = LayoutSnapshot::new(LayoutSize::new(320.0, 200.0));
        layout.nodes.insert(
            input,
            geometry(
                LayoutRect::new(10.0, 20.0, 180.0, 36.0),
                LayoutSize::new(180.0, 36.0),
            ),
        );
        let mut runtime = Runtime::default();
        runtime.runtime_state.interaction.set_focused(Some(input));
        runtime
            .runtime_state
            .text_edit
            .sync_from_runtime(input, "browser", None, None);
        runtime.runtime_state.text_edit.set_caret(input, 7, Some(2));

        let snapshot = SemanticSnapshot::build(&ir, &layout, &runtime);
        assert_eq!(snapshot.roots, vec![input]);
        assert_eq!(snapshot.focused, Some(input));
        assert_eq!(snapshot.nodes[0].id, input);
        assert_eq!(snapshot.nodes[0].value.as_deref(), Some("browser"));
        assert_eq!(snapshot.nodes[0].selection, Some((2, 7)));
    }

    #[test]
    fn snapshot_applies_scroll_and_clipping_to_visual_bounds() {
        let scroll = WidgetId::from_u128(1);
        let button = WidgetId::from_u128(2);
        let mut ir = CoreIR::new();
        add_node(
            &mut ir,
            button,
            Op::Semantics(Semantics {
                role: Role::Button,
                label: Some("Continue".into()),
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        add_node(
            &mut ir,
            scroll,
            Op::Layout(LayoutOp::Scroll {
                direction: FlexDirection::Column,
                show_scrollbar: true,
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
            }),
            vec![button],
        );
        ir.root = Some(scroll);
        let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
        layout.nodes.insert(
            scroll,
            geometry(
                LayoutRect::new(0.0, 0.0, 100.0, 100.0),
                LayoutSize::new(100.0, 400.0),
            ),
        );
        layout.nodes.insert(
            button,
            geometry(
                LayoutRect {
                    origin: LayoutPoint::new(0.0, 150.0),
                    size: LayoutSize::new(80.0, 30.0),
                },
                LayoutSize::new(80.0, 30.0),
            ),
        );
        let mut runtime = Runtime::default();
        runtime.runtime_state.scroll = ScrollStateMap::default();
        runtime.runtime_state.scroll.set_offset(scroll, 120.0);

        let snapshot = SemanticSnapshot::build(&ir, &layout, &runtime);
        assert_eq!(
            snapshot.nodes[0].bounds,
            Some(LayoutRect::new(0.0, 30.0, 80.0, 30.0))
        );
        assert_eq!(snapshot.nodes[0].visible_bounds, snapshot.nodes[0].bounds);
    }

    #[test]
    fn snapshot_keeps_parent_before_children_for_browser_tab_order() {
        let parent = WidgetId::from_u128(11);
        let child = WidgetId::from_u128(12);
        let mut ir = CoreIR::new();
        add_node(
            &mut ir,
            child,
            Op::Semantics(Semantics {
                role: Role::Button,
                label: Some("Child".into()),
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        add_node(
            &mut ir,
            parent,
            Op::Semantics(Semantics {
                role: Role::Dialog,
                label: Some("Parent".into()),
                focusable: true,
                ..Semantics::default()
            }),
            vec![child],
        );
        ir.root = Some(parent);

        let snapshot = SemanticSnapshot::build(
            &ir,
            &LayoutSnapshot::new(LayoutSize::new(320.0, 200.0)),
            &Runtime::default(),
        );

        assert_eq!(
            snapshot
                .nodes
                .iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![parent, child]
        );
        assert_eq!(snapshot.nodes[0].children, vec![child]);
        assert_eq!(snapshot.nodes[1].parent, Some(parent));
    }

    #[test]
    fn snapshot_keeps_pending_browser_edits_until_the_model_acknowledges_them() {
        let input = WidgetId::from_u128(21);
        let mut ir = CoreIR::new();
        add_node(
            &mut ir,
            input,
            Op::Semantics(Semantics {
                role: Role::TextInput,
                value: Some("model".into()),
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.root = Some(input);
        let mut runtime = Runtime::default();
        let state = runtime.runtime_state.text_edit.get_mut_or_default(input);
        state.apply_edit(0..0, "browser", 7, 7);

        let snapshot = SemanticSnapshot::build(
            &ir,
            &LayoutSnapshot::new(LayoutSize::new(320.0, 200.0)),
            &runtime,
        );

        assert_eq!(snapshot.nodes[0].value.as_deref(), Some("browser"));
    }
}
