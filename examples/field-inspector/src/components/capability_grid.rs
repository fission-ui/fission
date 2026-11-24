use crate::components::capability_cell::CapabilityCell;
use crate::components::ui::ResponsiveGrid;
use crate::model::{CapabilityLine, FieldInspectorState};
use fission::prelude::*;

const CAPABILITY_MIN_WIDTH: f32 = 280.0;

fission_component! {
    pub CapabilityGrid<FieldInspectorState> { lines: Vec<CapabilityLine> }
    view(grid, _state, _cx) =>
        ResponsiveGrid([
            for line in grid.lines.iter().cloned() {
                CapabilityCell(line.clone(), key: line.id)
            },
        ], CAPABILITY_MIN_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn semantic_ids(lines: Vec<CapabilityLine>) -> BTreeMap<String, WidgetId> {
        let mut harness = fission_test::TestHarness::new(FieldInspectorState::default())
            .with_root_widget(CapabilityGrid::new(lines));
        harness.pump().expect("capability grid frame");
        harness
            .last_ir
            .as_ref()
            .unwrap()
            .nodes
            .values()
            .filter_map(|node| match &node.op {
                Op::Semantics(semantics) => semantics
                    .identifier
                    .as_ref()
                    .filter(|id| id.starts_with("field-inspector.capability."))
                    .map(|id| (id.clone(), node.id)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn capability_identity_is_unique_and_does_not_follow_display_labels() {
        let lines = FieldInspectorState::default().capability_lines();
        let ids = lines.iter().map(|line| line.id).collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), lines.len());
        let count = lines.len();
        let original = semantic_ids(lines.clone());
        assert_eq!(original.len(), count);

        let translated = lines
            .into_iter()
            .rev()
            .map(|mut line| {
                // Equal translated labels must neither collide nor alter identity.
                line.title = "Translated capability";
                line
            })
            .collect();
        assert_eq!(semantic_ids(translated), original);
    }
}
