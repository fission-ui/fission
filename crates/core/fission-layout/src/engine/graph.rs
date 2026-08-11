use fission_ir::WidgetId;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::{BoxConstraints, LayoutInputNode, LayoutSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MeasureCacheKey {
    node_id: u128,
    min_w: u32,
    max_w: u32,
    min_h: u32,
    max_h: u32,
}

impl MeasureCacheKey {
    pub(super) fn new(node_id: WidgetId, constraints: BoxConstraints) -> Self {
        Self {
            node_id: node_id.as_u128(),
            min_w: constraints.min_w.to_bits(),
            max_w: constraints.max_w.to_bits(),
            min_h: constraints.min_h.to_bits(),
            max_h: constraints.max_h.to_bits(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct LayoutGraphValidationState {
    duplicate_nodes: Vec<WidgetId>,
    missing_parent_refs: Vec<(WidgetId, WidgetId)>,
    missing_child_refs: Vec<(WidgetId, WidgetId)>,
    parent_child_mismatches: Vec<(WidgetId, WidgetId, Option<WidgetId>)>,
    cycle_nodes: Vec<WidgetId>,
    root_nodes: Vec<WidgetId>,
}

impl LayoutGraphValidationState {
    pub(super) fn first_error(&self) -> Option<anyhow::Error> {
        if let Some(node_id) = self.duplicate_nodes.first() {
            return Some(anyhow::anyhow!(
                "[layout] duplicate node id encountered during graph build: {:?}",
                node_id
            ));
        }
        if let Some((node_id, parent_id)) = self.missing_parent_refs.first() {
            return Some(anyhow::anyhow!(
                "[layout] node {:?} references missing parent {:?}",
                node_id,
                parent_id
            ));
        }
        if let Some((node_id, child_id)) = self.missing_child_refs.first() {
            return Some(anyhow::anyhow!(
                "[layout] node {:?} references missing child {:?}",
                node_id,
                child_id
            ));
        }
        if let Some((parent_id, child_id, actual_parent)) = self.parent_child_mismatches.first() {
            return Some(anyhow::anyhow!(
                "[layout] parent/child mismatch parent={:?} child={:?} child.parent_id={:?}",
                parent_id,
                child_id,
                actual_parent
            ));
        }
        if let Some(node_id) = self.cycle_nodes.first() {
            return Some(anyhow::anyhow!(
                "[layout] cycle detected while rebuilding graph at {:?}",
                node_id
            ));
        }
        None
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct LayoutGraphState {
    pub(super) graph_version: u64,
    pub(super) last_layout_version: Option<u64>,
    pub(super) node_order: Vec<WidgetId>,
    pub(super) node_fingerprints: HashMap<WidgetId, u64>,
    pub(super) nodes: HashMap<WidgetId, LayoutInputNode>,
    pub(super) parents: HashMap<WidgetId, Option<WidgetId>>,
    pub(super) children: HashMap<WidgetId, Vec<WidgetId>>,
    pub(super) roots: Vec<WidgetId>,
    pub(super) validation: LayoutGraphValidationState,
}

#[derive(Debug, Clone, Default)]
pub(super) struct IncrementalLayoutReuseState {
    pub(super) previous_snapshot: LayoutSnapshot,
    pub(super) dirty_ancestors: HashSet<WidgetId>,
}

impl LayoutGraphState {
    pub(super) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub(super) fn mark_layout_complete(&mut self) {
        self.last_layout_version = Some(self.graph_version);
    }

    pub(super) fn matches_input_nodes(&self, input_nodes: &[LayoutInputNode]) -> bool {
        if self.nodes.len() != input_nodes.len() || self.node_order.len() != input_nodes.len() {
            return false;
        }

        for (expected_id, node) in self.node_order.iter().zip(input_nodes.iter()) {
            if *expected_id != node.id {
                return false;
            }
            let Some(existing) = self.node_fingerprints.get(&node.id) else {
                return false;
            };
            if *existing != layout_input_fingerprint(node) {
                return false;
            }
        }

        true
    }

    pub(super) fn from_input_nodes(input_nodes: &[LayoutInputNode], version: u64) -> Self {
        let mut state = Self {
            graph_version: version,
            ..Self::default()
        };
        state.replace_all_nodes(input_nodes);
        state
    }

    fn replace_all_nodes(&mut self, input_nodes: &[LayoutInputNode]) {
        self.node_order.clear();
        self.node_fingerprints.clear();
        self.nodes.clear();
        self.last_layout_version = None;

        let mut validation = LayoutGraphValidationState::default();
        let mut seen = HashSet::new();
        for node in input_nodes {
            if !seen.insert(node.id) {
                validation.duplicate_nodes.push(node.id);
            } else {
                self.node_order.push(node.id);
            }
            self.node_fingerprints
                .insert(node.id, layout_input_fingerprint(node));
            self.nodes.insert(node.id, node.clone());
        }

        self.rebuild_topology(validation);
    }

    pub(super) fn update_nodes(&mut self, input_nodes: &[LayoutInputNode]) {
        let mut validation = LayoutGraphValidationState::default();
        let mut seen = HashSet::new();
        let mut next_order = Vec::with_capacity(input_nodes.len());
        let mut next_fingerprints = HashMap::with_capacity(input_nodes.len());
        let mut next_nodes = HashMap::with_capacity(input_nodes.len());

        for node in input_nodes {
            if !seen.insert(node.id) {
                validation.duplicate_nodes.push(node.id);
                continue;
            }
            next_order.push(node.id);
            next_fingerprints.insert(node.id, layout_input_fingerprint(node));
            next_nodes.insert(node.id, node.clone());
        }

        self.node_order = next_order;
        self.node_fingerprints = next_fingerprints;
        self.nodes = next_nodes;
        self.last_layout_version = None;
        self.rebuild_topology(validation);
    }

    fn rebuild_topology(&mut self, mut validation: LayoutGraphValidationState) {
        self.parents.clear();
        self.children.clear();
        self.roots.clear();

        for node_id in &self.node_order {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            self.parents.insert(*node_id, node.parent_id);
            self.children.insert(*node_id, node.children_ids.clone());
            if node.parent_id.is_none() {
                self.roots.push(*node_id);
            } else if let Some(parent_id) = node.parent_id {
                if !self.nodes.contains_key(&parent_id) {
                    validation.missing_parent_refs.push((*node_id, parent_id));
                }
            }
        }

        for node_id in &self.node_order {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            for child_id in &node.children_ids {
                let Some(child) = self.nodes.get(child_id) else {
                    validation.missing_child_refs.push((*node_id, *child_id));
                    continue;
                };
                if child.parent_id != Some(*node_id) {
                    validation
                        .parent_child_mismatches
                        .push((*node_id, *child_id, child.parent_id));
                }
            }
        }

        validation.root_nodes = self.roots.clone();
        validation.cycle_nodes = self.detect_cycle_nodes();
        self.validation = validation;
    }

    pub(super) fn node(&self, node_id: WidgetId) -> Option<&LayoutInputNode> {
        self.nodes.get(&node_id)
    }

    pub(super) fn children_of(&self, node_id: WidgetId) -> &[WidgetId] {
        self.children
            .get(&node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn parent_of(&self, node_id: WidgetId) -> Option<WidgetId> {
        self.parents.get(&node_id).copied().flatten()
    }

    pub(super) fn ordered_nodes(&self) -> impl Iterator<Item = &LayoutInputNode> {
        self.node_order
            .iter()
            .filter_map(|node_id| self.nodes.get(node_id))
    }

    fn detect_cycle_nodes(&self) -> Vec<WidgetId> {
        fn dfs(
            node_id: WidgetId,
            children: &HashMap<WidgetId, Vec<WidgetId>>,
            visited: &mut HashSet<WidgetId>,
            stack: &mut HashSet<WidgetId>,
            cycle_nodes: &mut Vec<WidgetId>,
        ) {
            if stack.contains(&node_id) {
                cycle_nodes.push(node_id);
                return;
            }
            if !visited.insert(node_id) {
                return;
            }

            stack.insert(node_id);
            if let Some(child_nodes) = children.get(&node_id) {
                for child_id in child_nodes {
                    dfs(*child_id, children, visited, stack, cycle_nodes);
                }
            }
            stack.remove(&node_id);
        }

        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let mut cycle_nodes = Vec::new();
        for node_id in &self.node_order {
            dfs(
                *node_id,
                &self.children,
                &mut visited,
                &mut stack,
                &mut cycle_nodes,
            );
        }
        cycle_nodes.sort_by_key(|node_id| node_id.as_u128());
        cycle_nodes.dedup();
        cycle_nodes
    }
}
fn layout_input_fingerprint(node: &LayoutInputNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    format!("{node:?}").hash(&mut hasher);
    hasher.finish()
}
