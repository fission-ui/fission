use fission_ir::op::{AlignItems, JustifyContent};
use fission_ir::LayoutDirection;

use super::LayoutEngine;

pub(super) fn resolve_horizontal_flex_policy(
    is_row: bool,
    direction: LayoutDirection,
    justification: JustifyContent,
) -> (bool, JustifyContent) {
    if is_row {
        (
            direction == LayoutDirection::RightToLeft,
            direction.resolve_horizontal_justification(justification),
        )
    } else {
        (false, justification)
    }
}

pub(super) fn resolve_cross_axis_alignment(
    is_row: bool,
    direction: LayoutDirection,
    alignment: AlignItems,
) -> AlignItems {
    if is_row || direction == LayoutDirection::LeftToRight {
        return alignment;
    }

    match alignment {
        AlignItems::Start => AlignItems::End,
        AlignItems::End => AlignItems::Start,
        alignment => alignment,
    }
}

pub(super) fn reverse_flex_lines(
    is_row: bool,
    wrap_reverse: bool,
    direction: LayoutDirection,
) -> bool {
    wrap_reverse ^ (!is_row && direction == LayoutDirection::RightToLeft)
}

impl LayoutEngine {
    /// Returns an engine configured for the application's logical direction.
    pub fn with_layout_direction(mut self, direction: LayoutDirection) -> Self {
        self.set_layout_direction(direction);
        self
    }

    /// Synchronizes the logical direction used by subsequent layout passes.
    pub fn set_layout_direction(&mut self, direction: LayoutDirection) {
        if self.layout_direction != direction {
            self.layout_direction = direction;
            self.layout_direction_changed = true;
            self.incremental_reuse = None;
        }
    }

    pub(super) fn prepare_directional_layout(&mut self) {
        if self.layout_direction_changed {
            // Direction changes can reposition every horizontal descendant even
            // when retained node fingerprints are otherwise unchanged.
            self.incremental_reuse = None;
            self.layout_direction_changed = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use fission_ir::op::{AlignItems, FlexDirection, FlexWrap};
    use fission_ir::{LayoutOp, WidgetId};

    use super::*;
    use crate::{LayoutInputNode, LayoutPoint, LayoutSize};

    fn node(
        id: WidgetId,
        parent_id: Option<WidgetId>,
        children_ids: Vec<WidgetId>,
        op: LayoutOp,
    ) -> LayoutInputNode {
        let (width, height, flex_grow, flex_shrink) = match &op {
            LayoutOp::Box {
                width,
                height,
                flex_grow,
                flex_shrink,
                ..
            } => (*width, *height, *flex_grow, *flex_shrink),
            _ => (None, None, 0.0, 1.0),
        };
        LayoutInputNode {
            id,
            parent_id,
            op,
            children_ids,
            debug_name: format!("node-{}", id.as_u128()),
            width,
            height,
            flex_grow,
            flex_shrink,
            rich_text: None,
        }
    }

    fn fixed_box(id: WidgetId, parent_id: WidgetId, width: f32, height: f32) -> LayoutInputNode {
        node(
            id,
            Some(parent_id),
            Vec::new(),
            LayoutOp::Box {
                width: Some(width),
                height: Some(height),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            },
        )
    }

    #[test]
    fn right_to_left_wrapped_rows_preserve_logical_line_grouping() {
        let root = WidgetId::from_u128(80);
        let first = WidgetId::from_u128(81);
        let second = WidgetId::from_u128(82);
        let third = WidgetId::from_u128(83);
        let fixed_box = |id, width| {
            node(
                id,
                Some(root),
                Vec::new(),
                LayoutOp::Box {
                    width: Some(width),
                    height: Some(20.0),
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                },
            )
        };
        let nodes = vec![
            node(
                root,
                None,
                vec![first, second, third],
                LayoutOp::Flex {
                    direction: FlexDirection::Row,
                    wrap: FlexWrap::Wrap,
                    flex_grow: 0.0,
                    flex_shrink: 1.0,
                    padding: [0.0; 4],
                    gap: Some(0.0),
                    line_gap: None,
                    align_items: AlignItems::Start,
                    justify_content: JustifyContent::Start,
                },
            ),
            fixed_box(first, 60.0),
            fixed_box(second, 40.0),
            fixed_box(third, 40.0),
        ];

        let mut engine = LayoutEngine::new().with_layout_direction(LayoutDirection::RightToLeft);
        let snapshot = engine
            .compute_layout(&nodes, root, LayoutSize::new(100.0, 100.0), &|_| 0.0)
            .expect("right-to-left wrapped layout");

        assert_eq!(
            snapshot.get_node_rect(first).unwrap().origin,
            LayoutPoint::new(40.0, 0.0)
        );
        assert_eq!(
            snapshot.get_node_rect(second).unwrap().origin,
            LayoutPoint::new(0.0, 0.0)
        );
        assert_eq!(
            snapshot.get_node_rect(third).unwrap().origin,
            LayoutPoint::new(60.0, 20.0)
        );
    }

    #[test]
    fn right_to_left_columns_resolve_logical_cross_axis_alignment() {
        let root = WidgetId::from_u128(90);
        let child = WidgetId::from_u128(91);
        let nodes = vec![
            node(
                root,
                None,
                vec![child],
                LayoutOp::Flex {
                    direction: FlexDirection::Column,
                    wrap: FlexWrap::NoWrap,
                    flex_grow: 0.0,
                    flex_shrink: 1.0,
                    padding: [0.0; 4],
                    gap: Some(0.0),
                    line_gap: None,
                    align_items: AlignItems::Start,
                    justify_content: JustifyContent::Start,
                },
            ),
            fixed_box(child, root, 20.0, 20.0),
        ];

        let mut engine = LayoutEngine::new().with_layout_direction(LayoutDirection::RightToLeft);
        let snapshot = engine
            .compute_layout(&nodes, root, LayoutSize::new(100.0, 100.0), &|_| 0.0)
            .expect("right-to-left column layout");

        assert_eq!(
            snapshot.get_node_rect(child).unwrap().origin,
            LayoutPoint::new(80.0, 0.0)
        );
    }

    #[test]
    fn right_to_left_wrapped_columns_begin_at_logical_cross_start() {
        let root = WidgetId::from_u128(100);
        let first = WidgetId::from_u128(101);
        let second = WidgetId::from_u128(102);
        let third = WidgetId::from_u128(103);
        let nodes = vec![
            node(
                root,
                None,
                vec![first, second, third],
                LayoutOp::Flex {
                    direction: FlexDirection::Column,
                    wrap: FlexWrap::Wrap,
                    flex_grow: 0.0,
                    flex_shrink: 1.0,
                    padding: [0.0; 4],
                    gap: Some(0.0),
                    line_gap: None,
                    align_items: AlignItems::Start,
                    justify_content: JustifyContent::Start,
                },
            ),
            fixed_box(first, root, 20.0, 60.0),
            fixed_box(second, root, 30.0, 40.0),
            fixed_box(third, root, 40.0, 40.0),
        ];

        let mut engine = LayoutEngine::new().with_layout_direction(LayoutDirection::RightToLeft);
        let snapshot = engine
            .compute_layout(&nodes, root, LayoutSize::new(100.0, 100.0), &|_| 0.0)
            .expect("right-to-left wrapped column layout");

        assert_eq!(
            snapshot.get_node_rect(first).unwrap().origin,
            LayoutPoint::new(80.0, 0.0)
        );
        assert_eq!(
            snapshot.get_node_rect(second).unwrap().origin,
            LayoutPoint::new(70.0, 60.0)
        );
        assert_eq!(
            snapshot.get_node_rect(third).unwrap().origin,
            LayoutPoint::new(30.0, 0.0)
        );
    }
}
