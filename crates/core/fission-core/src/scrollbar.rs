use crate::env::ScrollStateMap;
use fission_ir::{CoreIR, FlexDirection, LayoutOp, Op, WidgetId};
use fission_layout::{LayoutPoint, LayoutRect, LayoutSnapshot};

pub const SCROLLBAR_INSET: f32 = 2.0;
pub const SCROLLBAR_THICKNESS: f32 = 6.0;
pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollbarGeometry {
    pub node_id: WidgetId,
    pub axis: ScrollbarAxis,
    pub rail_rect: LayoutRect,
    pub thumb_rect: LayoutRect,
    pub offset: f32,
    pub max_offset: f32,
    pub track_travel: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarHitKind {
    Thumb,
    Rail,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollbarHit {
    pub geometry: ScrollbarGeometry,
    pub kind: ScrollbarHitKind,
    pub pointer_to_thumb_start: f32,
    pub layout_point: LayoutPoint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollbarDragState {
    pub node_id: WidgetId,
    pub pointer_to_thumb_start: f32,
}

pub fn scrollbar_geometry_for_node(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    node_id: WidgetId,
) -> Option<ScrollbarGeometry> {
    let node = ir.nodes.get(&node_id)?;
    let Op::Layout(LayoutOp::Scroll {
        direction,
        show_scrollbar,
        ..
    }) = &node.op
    else {
        return None;
    };
    if !show_scrollbar {
        return None;
    }

    let geom = layout.get_node_geometry(node_id)?;
    let rect = geom.rect;
    let (axis, viewport_extent, content_extent, rail_rect) = match direction {
        FlexDirection::Column => {
            let rail_extent = (rect.size.height - SCROLLBAR_INSET * 2.0).max(0.0);
            (
                ScrollbarAxis::Vertical,
                rect.size.height,
                geom.content_size.height,
                LayoutRect::new(
                    rect.origin.x + rect.size.width - SCROLLBAR_THICKNESS - SCROLLBAR_INSET,
                    rect.origin.y + SCROLLBAR_INSET,
                    SCROLLBAR_THICKNESS,
                    rail_extent,
                ),
            )
        }
        FlexDirection::Row => {
            let rail_extent = (rect.size.width - SCROLLBAR_INSET * 2.0).max(0.0);
            (
                ScrollbarAxis::Horizontal,
                rect.size.width,
                geom.content_size.width,
                LayoutRect::new(
                    rect.origin.x + SCROLLBAR_INSET,
                    rect.origin.y + rect.size.height - SCROLLBAR_THICKNESS - SCROLLBAR_INSET,
                    rail_extent,
                    SCROLLBAR_THICKNESS,
                ),
            )
        }
    };

    if viewport_extent <= 0.0 || content_extent <= viewport_extent + 0.5 {
        return None;
    }

    let rail_extent = axis_extent(axis, rail_rect);
    if rail_extent <= 0.0 {
        return None;
    }

    let max_offset = (content_extent - viewport_extent).max(0.0);
    let offset = scroll_map.get_offset(node_id).clamp(0.0, max_offset);
    let min_thumb = SCROLLBAR_MIN_THUMB.min(rail_extent);
    let thumb_extent =
        ((viewport_extent / content_extent) * rail_extent).clamp(min_thumb, rail_extent);
    let track_travel = (rail_extent - thumb_extent).max(0.0);
    let thumb_start = axis_start(axis, rail_rect)
        + if max_offset > 0.0 && track_travel > 0.0 {
            (offset / max_offset) * track_travel
        } else {
            0.0
        };

    let thumb_rect = match axis {
        ScrollbarAxis::Vertical => LayoutRect::new(
            rail_rect.origin.x,
            thumb_start,
            SCROLLBAR_THICKNESS,
            thumb_extent,
        ),
        ScrollbarAxis::Horizontal => LayoutRect::new(
            thumb_start,
            rail_rect.origin.y,
            thumb_extent,
            SCROLLBAR_THICKNESS,
        ),
    };

    Some(ScrollbarGeometry {
        node_id,
        axis,
        rail_rect,
        thumb_rect,
        offset,
        max_offset,
        track_travel,
    })
}

pub fn scrollbar_hit_test(
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    point: LayoutPoint,
) -> Option<ScrollbarHit> {
    let root = ir.root?;
    scrollbar_hit_test_recursive(root, ir, layout, scroll_map, point)
}

pub fn scrollbar_drag_offset(geometry: ScrollbarGeometry, point: LayoutPoint) -> f32 {
    scrollbar_drag_offset_with_grab(geometry, point, geometry.thumb_extent() * 0.5)
}

pub fn scrollbar_point_for_node(
    ir: &CoreIR,
    scroll_map: &ScrollStateMap,
    node_id: WidgetId,
    mut point: LayoutPoint,
) -> LayoutPoint {
    let mut current = ir.nodes.get(&node_id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        let Some(parent) = ir.nodes.get(&parent_id) else {
            break;
        };
        if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &parent.op {
            let offset = scroll_map.get_offset(parent_id);
            match direction {
                FlexDirection::Column => point.y += offset,
                FlexDirection::Row => point.x += offset,
            }
        }
        current = parent.parent;
    }
    point
}

pub fn scrollbar_drag_offset_with_grab(
    geometry: ScrollbarGeometry,
    point: LayoutPoint,
    pointer_to_thumb_start: f32,
) -> f32 {
    if geometry.track_travel <= 0.0 || geometry.max_offset <= 0.0 {
        return 0.0;
    }
    let rail_start = axis_start(geometry.axis, geometry.rail_rect);
    let pointer_axis = point_axis(geometry.axis, point);
    let requested_thumb_start = pointer_axis - pointer_to_thumb_start;
    let normalized = ((requested_thumb_start - rail_start) / geometry.track_travel).clamp(0.0, 1.0);
    normalized * geometry.max_offset
}

impl ScrollbarGeometry {
    pub fn thumb_extent(self) -> f32 {
        axis_extent(self.axis, self.thumb_rect)
    }
}

fn scrollbar_hit_test_recursive(
    node_id: WidgetId,
    ir: &CoreIR,
    layout: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    point: LayoutPoint,
) -> Option<ScrollbarHit> {
    let node = ir.nodes.get(&node_id)?;
    let geom = layout.get_node_geometry(node_id)?;
    let is_clip_container = matches!(
        node.op,
        Op::Layout(LayoutOp::Clip { .. }) | Op::Layout(LayoutOp::Scroll { .. })
    );
    if is_clip_container && !geom.rect.contains(point) {
        return None;
    }

    if let Some(geometry) = scrollbar_geometry_for_node(ir, layout, scroll_map, node_id) {
        if geometry.thumb_rect.contains(point) {
            return Some(ScrollbarHit {
                geometry,
                kind: ScrollbarHitKind::Thumb,
                pointer_to_thumb_start: point_axis(geometry.axis, point)
                    - axis_start(geometry.axis, geometry.thumb_rect),
                layout_point: point,
            });
        }
        if geometry.rail_rect.contains(point) {
            return Some(ScrollbarHit {
                geometry,
                kind: ScrollbarHitKind::Rail,
                pointer_to_thumb_start: geometry.thumb_extent() * 0.5,
                layout_point: point,
            });
        }
    }

    let mut child_point = point;
    if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &node.op {
        let offset = scroll_map.get_offset(node_id);
        match direction {
            FlexDirection::Column => child_point.y += offset,
            FlexDirection::Row => child_point.x += offset,
        }
    }

    for child_id in node.children.iter().rev() {
        if let Some(hit) =
            scrollbar_hit_test_recursive(*child_id, ir, layout, scroll_map, child_point)
        {
            return Some(hit);
        }
    }

    None
}

fn axis_start(axis: ScrollbarAxis, rect: LayoutRect) -> f32 {
    match axis {
        ScrollbarAxis::Horizontal => rect.origin.x,
        ScrollbarAxis::Vertical => rect.origin.y,
    }
}

fn axis_extent(axis: ScrollbarAxis, rect: LayoutRect) -> f32 {
    match axis {
        ScrollbarAxis::Horizontal => rect.size.width,
        ScrollbarAxis::Vertical => rect.size.height,
    }
}

fn point_axis(axis: ScrollbarAxis, point: LayoutPoint) -> f32 {
    match axis {
        ScrollbarAxis::Horizontal => point.x,
        ScrollbarAxis::Vertical => point.y,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        scrollbar_drag_offset_with_grab, scrollbar_geometry_for_node, scrollbar_hit_test,
        scrollbar_point_for_node, ScrollbarAxis, ScrollbarHitKind,
    };
    use crate::env::ScrollStateMap;
    use fission_ir::{CompositeStyle, CoreIR, CoreNode, FlexDirection, LayoutOp, Op, WidgetId};
    use fission_layout::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot};

    #[test]
    fn vertical_scrollbar_geometry_tracks_offset_inside_viewport() {
        let (ir, mut layout, scroll) = scroll_tree();
        layout.nodes.insert(
            scroll,
            LayoutNodeGeometry {
                rect: LayoutRect::new(10.0, 20.0, 100.0, 200.0),
                content_size: LayoutSize::new(100.0, 600.0),
            },
        );
        let mut scroll_map = ScrollStateMap::default();
        scroll_map.set_offset(scroll, 200.0);

        let geometry =
            scrollbar_geometry_for_node(&ir, &layout, &scroll_map, scroll).expect("scrollbar");

        assert_eq!(geometry.axis, ScrollbarAxis::Vertical);
        assert_eq!(geometry.rail_rect.origin.x, 102.0);
        assert_eq!(geometry.rail_rect.origin.y, 22.0);
        assert!(geometry.thumb_rect.origin.y > geometry.rail_rect.origin.y);
        assert!(geometry.thumb_rect.bottom() <= geometry.rail_rect.bottom());
    }

    #[test]
    fn scrollbar_hit_test_prioritizes_thumb_chrome() {
        let (ir, mut layout, scroll) = scroll_tree();
        layout.nodes.insert(
            scroll,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 0.0, 100.0, 200.0),
                content_size: LayoutSize::new(100.0, 600.0),
            },
        );

        let hit = scrollbar_hit_test(
            &ir,
            &layout,
            &ScrollStateMap::default(),
            LayoutPoint::new(97.0, 8.0),
        )
        .expect("scrollbar hit");

        assert_eq!(hit.kind, ScrollbarHitKind::Thumb);
        assert_eq!(hit.geometry.node_id, scroll);
    }

    #[test]
    fn scrollbar_drag_maps_thumb_position_to_offset() {
        let (ir, mut layout, scroll) = scroll_tree();
        layout.nodes.insert(
            scroll,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 0.0, 100.0, 200.0),
                content_size: LayoutSize::new(100.0, 600.0),
            },
        );
        let geometry =
            scrollbar_geometry_for_node(&ir, &layout, &ScrollStateMap::default(), scroll).unwrap();

        let offset = scrollbar_drag_offset_with_grab(geometry, LayoutPoint::new(97.0, 198.0), 0.0);

        assert!((offset - geometry.max_offset).abs() <= 0.01);
    }

    #[test]
    fn nested_scrollbar_hit_uses_target_layout_coordinates() {
        let parent = WidgetId::derived(71, &[0]);
        let child = WidgetId::derived(71, &[1]);
        let mut ir = CoreIR::new();
        ir.add_node(
            child,
            Op::Layout(LayoutOp::Scroll {
                direction: FlexDirection::Row,
                show_scrollbar: true,
                width: Some(100.0),
                height: Some(50.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
            vec![],
        );
        ir.add_node(
            parent,
            Op::Layout(LayoutOp::Scroll {
                direction: FlexDirection::Column,
                show_scrollbar: true,
                width: Some(120.0),
                height: Some(120.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
            vec![child],
        );
        ir.set_root(parent);

        let mut layout = LayoutSnapshot::new(LayoutSize::new(120.0, 120.0));
        layout.nodes.insert(
            parent,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 0.0, 120.0, 120.0),
                content_size: LayoutSize::new(120.0, 320.0),
            },
        );
        layout.nodes.insert(
            child,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 160.0, 100.0, 50.0),
                content_size: LayoutSize::new(300.0, 50.0),
            },
        );
        let mut scroll_map = ScrollStateMap::default();
        scroll_map.set_offset(parent, 100.0);

        let visual_rail_point = LayoutPoint::new(50.0, 104.0);
        let hit =
            scrollbar_hit_test(&ir, &layout, &scroll_map, visual_rail_point).expect("child rail");

        assert_eq!(hit.geometry.node_id, child);
        assert_eq!(hit.kind, ScrollbarHitKind::Rail);
        assert_eq!(
            hit.layout_point,
            scrollbar_point_for_node(&ir, &scroll_map, child, visual_rail_point)
        );
        assert!(
            hit.geometry.rail_rect.contains(hit.layout_point),
            "hit point must be in the target scrollbar's layout coordinate space"
        );
    }

    fn scroll_tree() -> (CoreIR, LayoutSnapshot, WidgetId) {
        let scroll = WidgetId::derived(70, &[1]);
        let mut ir = CoreIR::default();
        ir.nodes.insert(
            scroll,
            CoreNode {
                id: scroll,
                parent: None,
                children: Vec::new(),
                op: Op::Layout(LayoutOp::Scroll {
                    direction: FlexDirection::Column,
                    show_scrollbar: true,
                    width: Some(100.0),
                    height: Some(200.0),
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                }),
                composite: CompositeStyle::default(),
                hash: 0,
            },
        );
        ir.set_root(scroll);
        (
            ir,
            LayoutSnapshot::new(LayoutSize::new(100.0, 200.0)),
            scroll,
        )
    }
}
