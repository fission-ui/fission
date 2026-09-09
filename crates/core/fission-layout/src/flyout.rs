use std::collections::HashMap;

use anyhow::Result;
use fission_ir::{
    FlyoutAlignment, FlyoutOptions, FlyoutPlacement, FlyoutWidth, LayoutDirection, LayoutOp,
    WidgetId,
};

use super::{
    BoxConstraints, LayoutEngine, LayoutGraphState, LayoutPoint, LayoutRect, LayoutSize,
    LayoutSnapshot, LayoutUnit, MeasureCacheKey, ScrollDataSource,
};

pub(super) fn root_position(
    viewport: LayoutSize,
    anchor: LayoutRect,
    content_extents: LayoutRect,
    options: FlyoutOptions,
) -> LayoutPoint {
    root_position_with_target(viewport, anchor, content_extents, options, None)
}

pub(super) fn root_position_with_target(
    viewport: LayoutSize,
    anchor: LayoutRect,
    content_extents: LayoutRect,
    options: FlyoutOptions,
    target_relative_center_y: Option<LayoutUnit>,
) -> LayoutPoint {
    let min_left = -content_extents.x();
    let max_left = viewport.width - content_extents.right();
    let desired_left = match options.alignment {
        FlyoutAlignment::Start => anchor.x() - content_extents.x(),
        FlyoutAlignment::End => anchor.right() - content_extents.right(),
    };
    let left = if max_left >= min_left {
        desired_left.clamp(min_left, max_left)
    } else {
        min_left
    };

    let gap = if options.gap.is_finite() {
        options.gap.max(0.0)
    } else {
        0.0
    };
    let below = anchor.bottom() + gap - content_extents.y();
    let above = anchor.y() - gap - content_extents.bottom();
    let min_top = -content_extents.y();
    let max_top = viewport.height - content_extents.bottom();
    let below_fits = below + content_extents.bottom() <= viewport.height;
    let above_fits = above + content_extents.y() >= 0.0;
    let preferred_top = target_relative_center_y
        .filter(|center| center.is_finite())
        .map(|center| anchor.y() + anchor.height() * 0.5 - center)
        .unwrap_or_else(|| match options.placement {
            FlyoutPlacement::Below => below,
            FlyoutPlacement::Above => above,
            FlyoutPlacement::Auto if below_fits => below,
            FlyoutPlacement::Auto if above_fits => above,
            FlyoutPlacement::Auto => {
                let room_below = (viewport.height - anchor.bottom() - gap).max(0.0);
                let room_above = (anchor.y() - gap).max(0.0);
                if room_below >= room_above {
                    below
                } else {
                    above
                }
            }
        });
    let top = if max_top >= min_top {
        preferred_top.clamp(min_top, max_top)
    } else {
        min_top
    };

    LayoutPoint::new(left, top)
}

fn child_constraints(
    constraints: BoxConstraints,
    anchor_width: Option<LayoutUnit>,
    width: FlyoutWidth,
) -> BoxConstraints {
    let Some(anchor_width) = anchor_width.filter(|width| width.is_finite()) else {
        return constraints;
    };
    let anchor_width = anchor_width.max(0.0).min(constraints.max_w);
    match width {
        FlyoutWidth::Content => constraints,
        FlyoutWidth::MatchAnchor => constraints.tighten(Some(anchor_width), None),
        FlyoutWidth::AtLeastAnchor => BoxConstraints {
            min_w: constraints.min_w.max(anchor_width),
            ..constraints
        },
    }
}

pub(super) fn absolute_overrides(
    graph_state: &LayoutGraphState,
    snapshot: &LayoutSnapshot,
    layout_direction: LayoutDirection,
    visual_location: &impl Fn(WidgetId) -> Option<LayoutPoint>,
) -> HashMap<WidgetId, (f32, f32)> {
    let mut overrides = HashMap::new();
    for node in graph_state.ordered_nodes() {
        let LayoutOp::Flyout {
            anchor,
            content,
            options,
        } = node.op
        else {
            continue;
        };
        let (Some(anchor_geometry), Some(content_geometry)) =
            (snapshot.nodes.get(&anchor), snapshot.nodes.get(&content))
        else {
            continue;
        };
        let (Some(anchor_absolute), Some(content_absolute)) =
            (visual_location(anchor), visual_location(content))
        else {
            continue;
        };

        let mut min_x: f32 = 0.0;
        let mut min_y: f32 = 0.0;
        let mut max_x = content_geometry.rect.width();
        let mut max_y = content_geometry.rect.height();
        let mut stack = vec![content];
        while let Some(current) = stack.pop() {
            if let (Some(geometry), Some(origin)) =
                (snapshot.nodes.get(&current), visual_location(current))
            {
                let relative_x = origin.x - content_absolute.x;
                let relative_y = origin.y - content_absolute.y;
                min_x = min_x.min(relative_x);
                min_y = min_y.min(relative_y);
                max_x = max_x.max(relative_x + geometry.rect.width());
                max_y = max_y.max(relative_y + geometry.rect.height());
            }
            stack.extend(graph_state.children_of(current).iter().copied());
        }

        let anchor_rect = LayoutRect::new(
            anchor_absolute.x,
            anchor_absolute.y,
            anchor_geometry.rect.width(),
            anchor_geometry.rect.height(),
        );
        let content_extents = LayoutRect::new(min_x, min_y, max_x - min_x, max_y - min_y);
        let mut resolved_options = options;
        resolved_options.alignment = layout_direction.resolve_flyout_alignment(options.alignment);
        let target_relative_center_y = options.alignment_target.and_then(|target| {
            let target_geometry = snapshot.nodes.get(&target)?;
            let target_origin = visual_location(target)?;
            Some(target_origin.y - content_absolute.y + target_geometry.rect.height() * 0.5)
        });
        let position = root_position_with_target(
            snapshot.viewport_size,
            anchor_rect,
            content_extents,
            resolved_options,
            target_relative_center_y,
        );
        overrides.insert(content, (position.x, position.y));
    }
    overrides
}

impl LayoutEngine {
    pub(super) fn apply_flyout_width_policies(
        &self,
        viewport_size: LayoutSize,
        snapshot: &mut LayoutSnapshot,
        measure_cache: &mut HashMap<MeasureCacheKey, LayoutSize>,
        scroll_source: &impl ScrollDataSource,
    ) -> Result<()> {
        // A flyout and its anchor can live in different portal branches. Apply
        // anchor-derived width after the first complete geometry pass so the
        // result does not depend on sibling traversal order.
        let policies = self
            .graph_state
            .ordered_nodes()
            .filter_map(|node| match node.op {
                LayoutOp::Flyout {
                    anchor,
                    content,
                    options,
                } if options.width != FlyoutWidth::Content => {
                    Some((node.id, anchor, content, options.width))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for (flyout, anchor, content, width) in policies {
            let Some(anchor_width) = snapshot
                .nodes
                .get(&anchor)
                .map(|geometry| geometry.rect.width())
            else {
                continue;
            };
            let Some(origin) = snapshot
                .nodes
                .get(&content)
                .map(|geometry| geometry.rect.origin)
            else {
                continue;
            };
            let content_constraints = child_constraints(
                BoxConstraints::loose(viewport_size.width, viewport_size.height),
                Some(anchor_width),
                width,
            );
            let content_size = self.layout_node_constraints(
                content,
                content_constraints,
                origin,
                &mut snapshot.nodes,
                &mut snapshot.constraints,
                measure_cache,
                scroll_source,
                true,
                0,
            )?;
            if let Some(geometry) = snapshot.nodes.get_mut(&flyout) {
                geometry.rect.size = content_size;
                geometry.content_size = content_size;
            }
        }
        Ok(())
    }

    pub(super) fn apply_flyout_absolute_overrides(
        &self,
        snapshot: &mut LayoutSnapshot,
        overrides: HashMap<WidgetId, (f32, f32)>,
    ) -> Result<()> {
        for (node_id, (absolute_x, absolute_y)) in overrides {
            let Some(current) = snapshot.nodes.get(&node_id) else {
                continue;
            };
            let dx = absolute_x - current.rect.origin.x;
            let dy = absolute_y - current.rect.origin.y;
            let mut stack = vec![(node_id, 0usize)];
            while let Some((current_id, depth)) = stack.pop() {
                if depth > Self::MAX_LAYOUT_RECURSION_DEPTH {
                    return Err(self.layout_depth_overflow(current_id, depth));
                }
                if let Some(geometry) = snapshot.nodes.get_mut(&current_id) {
                    geometry.rect.origin.x += dx;
                    geometry.rect.origin.y += dy;
                }
                for child_id in self.graph_state.children_of(current_id).iter().rev() {
                    stack.push((*child_id, depth + 1));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use fission_ir::LayoutOp;

    use super::*;
    use crate::LayoutInputNode;

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
            LayoutOp::StyledBox {
                flex_grow,
                flex_shrink,
                ..
            } => (None, None, *flex_grow, *flex_shrink),
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
    fn flyout_end_alignment_tracks_anchor_end_and_clamps_at_viewport_edge() {
        let aligned = root_position(
            LayoutSize::new(400.0, 300.0),
            LayoutRect::new(250.0, 40.0, 80.0, 32.0),
            LayoutRect::new(0.0, 0.0, 120.0, 60.0),
            FlyoutOptions::default()
                .with_alignment(FlyoutAlignment::End)
                .with_gap(4.0),
        );
        assert_eq!(aligned, LayoutPoint::new(210.0, 76.0));

        let clamped = root_position(
            LayoutSize::new(400.0, 300.0),
            LayoutRect::new(380.0, 40.0, 40.0, 32.0),
            LayoutRect::new(0.0, 0.0, 120.0, 60.0),
            FlyoutOptions::default()
                .with_alignment(FlyoutAlignment::End)
                .with_gap(4.0),
        );
        assert_eq!(clamped, LayoutPoint::new(280.0, 76.0));
    }

    #[test]
    fn logical_flyout_edges_resolve_against_layout_direction() {
        let root = WidgetId::from_u128(70);
        let positioned = WidgetId::from_u128(71);
        let anchor = WidgetId::from_u128(72);
        let flyout = WidgetId::from_u128(73);
        let content = WidgetId::from_u128(74);
        let nodes = vec![
            node(root, None, vec![positioned, flyout], LayoutOp::ZStack),
            node(
                positioned,
                Some(root),
                vec![anchor],
                LayoutOp::Positioned {
                    left: Some(200.0),
                    top: Some(40.0),
                    right: None,
                    bottom: None,
                    width: None,
                    height: None,
                },
            ),
            fixed_box(anchor, positioned, 80.0, 32.0),
            node(
                flyout,
                Some(root),
                vec![content],
                LayoutOp::Flyout {
                    anchor,
                    content,
                    options: FlyoutOptions::default(),
                },
            ),
            fixed_box(content, flyout, 120.0, 48.0),
        ];

        let mut left_to_right =
            LayoutEngine::new().with_layout_direction(LayoutDirection::LeftToRight);
        let left_to_right = left_to_right
            .compute_layout(&nodes, root, LayoutSize::new(400.0, 300.0), &|_| 0.0)
            .expect("left-to-right flyout layout");
        assert_eq!(
            left_to_right
                .get_node_rect(content)
                .expect("left-to-right content")
                .x(),
            200.0
        );

        let mut right_to_left =
            LayoutEngine::new().with_layout_direction(LayoutDirection::RightToLeft);
        let right_to_left = right_to_left
            .compute_layout(&nodes, root, LayoutSize::new(400.0, 300.0), &|_| 0.0)
            .expect("right-to-left flyout layout");
        assert_eq!(
            right_to_left
                .get_node_rect(content)
                .expect("right-to-left content")
                .x(),
            160.0
        );

        let mut end_nodes = nodes.clone();
        end_nodes
            .iter_mut()
            .find(|node| node.id == flyout)
            .expect("flyout input")
            .op = LayoutOp::Flyout {
            anchor,
            content,
            options: FlyoutOptions::default().with_alignment(FlyoutAlignment::End),
        };
        let mut left_to_right =
            LayoutEngine::new().with_layout_direction(LayoutDirection::LeftToRight);
        let left_to_right = left_to_right
            .compute_layout(&end_nodes, root, LayoutSize::new(400.0, 300.0), &|_| 0.0)
            .expect("left-to-right end-aligned flyout layout");
        assert_eq!(
            left_to_right
                .get_node_rect(content)
                .expect("left-to-right end content")
                .x(),
            160.0
        );

        let mut right_to_left =
            LayoutEngine::new().with_layout_direction(LayoutDirection::RightToLeft);
        let right_to_left = right_to_left
            .compute_layout(&end_nodes, root, LayoutSize::new(400.0, 300.0), &|_| 0.0)
            .expect("right-to-left end-aligned flyout layout");
        assert_eq!(
            right_to_left
                .get_node_rect(content)
                .expect("right-to-left end content")
                .x(),
            200.0
        );
    }

    #[test]
    fn flyout_auto_placement_uses_above_fallback_with_configured_gap() {
        let position = root_position(
            LayoutSize::new(400.0, 300.0),
            LayoutRect::new(80.0, 260.0, 120.0, 32.0),
            LayoutRect::new(0.0, 0.0, 120.0, 60.0),
            FlyoutOptions::default().with_gap(4.0),
        );

        assert_eq!(position, LayoutPoint::new(80.0, 196.0));
    }

    #[test]
    fn flyout_explicit_side_is_collision_clamped() {
        let position = root_position(
            LayoutSize::new(400.0, 300.0),
            LayoutRect::new(80.0, 260.0, 120.0, 32.0),
            LayoutRect::new(0.0, 0.0, 120.0, 60.0),
            FlyoutOptions::default()
                .with_placement(FlyoutPlacement::Below)
                .with_gap(4.0),
        );

        assert_eq!(position, LayoutPoint::new(80.0, 240.0));
    }

    #[test]
    fn flyout_width_policies_preserve_content_or_apply_anchor_width() {
        let root = WidgetId::from_u128(90);
        let positioned = WidgetId::from_u128(91);
        let anchor = WidgetId::from_u128(92);
        let flyout = WidgetId::from_u128(93);
        let content = WidgetId::from_u128(94);

        for (width_policy, intrinsic_width, expected_width) in [
            (FlyoutWidth::Content, 60.0, 60.0),
            (FlyoutWidth::MatchAnchor, 180.0, 120.0),
            (FlyoutWidth::AtLeastAnchor, 60.0, 120.0),
            (FlyoutWidth::AtLeastAnchor, 180.0, 180.0),
        ] {
            let nodes = vec![
                node(root, None, vec![positioned, flyout], LayoutOp::ZStack),
                node(
                    positioned,
                    Some(root),
                    vec![anchor],
                    LayoutOp::Positioned {
                        left: Some(50.0),
                        top: Some(40.0),
                        right: None,
                        bottom: None,
                        width: None,
                        height: None,
                    },
                ),
                fixed_box(anchor, positioned, 120.0, 32.0),
                node(
                    flyout,
                    Some(root),
                    vec![content],
                    LayoutOp::Flyout {
                        anchor,
                        content,
                        options: FlyoutOptions::default()
                            .with_width(width_policy)
                            .with_gap(4.0),
                    },
                ),
                fixed_box(content, flyout, intrinsic_width, 48.0),
            ];
            let mut engine = LayoutEngine::new();
            let snapshot = engine
                .compute_layout(&nodes, root, LayoutSize::new(400.0, 300.0), &|_| 0.0)
                .expect("flyout layout");
            let content_rect = snapshot.get_node_rect(content).expect("flyout content");

            assert_eq!(content_rect.width(), expected_width, "{width_policy:?}");
            assert_eq!(content_rect.x(), 50.0, "{width_policy:?}");
            assert_eq!(content_rect.y(), 76.0, "{width_policy:?}");
        }
    }

    #[test]
    fn flyout_alignment_target_centres_a_selected_descendant_on_the_anchor() {
        let root = WidgetId::from_u128(100);
        let anchor_position = WidgetId::from_u128(101);
        let anchor = WidgetId::from_u128(102);
        let flyout = WidgetId::from_u128(103);
        let content = WidgetId::from_u128(104);
        let target_position = WidgetId::from_u128(105);
        let target = WidgetId::from_u128(106);
        let nodes = vec![
            node(root, None, vec![anchor_position, flyout], LayoutOp::ZStack),
            node(
                anchor_position,
                Some(root),
                vec![anchor],
                LayoutOp::Positioned {
                    left: Some(100.0),
                    top: Some(120.0),
                    right: None,
                    bottom: None,
                    width: None,
                    height: None,
                },
            ),
            fixed_box(anchor, anchor_position, 120.0, 32.0),
            node(
                flyout,
                Some(root),
                vec![content],
                LayoutOp::Flyout {
                    anchor,
                    content,
                    options: FlyoutOptions::default()
                        .with_width(FlyoutWidth::MatchAnchor)
                        .with_alignment_target(target),
                },
            ),
            node(
                content,
                Some(flyout),
                vec![target_position],
                LayoutOp::Box {
                    width: Some(120.0),
                    height: Some(120.0),
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                },
            ),
            node(
                target_position,
                Some(content),
                vec![target],
                LayoutOp::Positioned {
                    left: Some(0.0),
                    top: Some(40.0),
                    right: None,
                    bottom: None,
                    width: None,
                    height: None,
                },
            ),
            fixed_box(target, target_position, 120.0, 32.0),
        ];

        let mut engine = LayoutEngine::new();
        let snapshot = engine
            .compute_layout(&nodes, root, LayoutSize::new(400.0, 300.0), &|_| 0.0)
            .expect("target-aligned flyout layout");
        let anchor_rect = snapshot.get_node_rect(anchor).expect("anchor");
        let target_rect = snapshot.get_node_rect(target).expect("target");

        assert_eq!(snapshot.get_node_rect(content).expect("content").y(), 80.0);
        assert_eq!(
            anchor_rect.y() + anchor_rect.height() * 0.5,
            target_rect.y() + target_rect.height() * 0.5
        );
    }
}
