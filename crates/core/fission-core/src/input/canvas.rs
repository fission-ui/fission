use fission_ir::{CanvasSelectionPolicy, CanvasTarget, CanvasTargetKind, WidgetId};
use fission_layout::{LayoutPoint, LayoutRect, LayoutSnapshot};

use crate::event::PointerKind;
use crate::input::viewport::ViewportStateMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasInteractionPhase {
    Start,
    Update,
    End,
    Activate,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasInteractionKind {
    SelectNode { node_id: u128 },
    MoveNode { node_id: u128 },
    ResizeNode { node_id: u128, handle: u8 },
    SelectEdge { edge_id: u128 },
    Marquee,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasInteraction {
    pub canvas_id: WidgetId,
    pub target_id: WidgetId,
    pub kind: CanvasInteractionKind,
    pub selection_policy: CanvasSelectionPolicy,
    pub phase: CanvasInteractionPhase,
    pub input_kind: PointerKind,
    /// Modifier bitmask (Shift=1, Alt=2, Ctrl=4, Super=8).
    pub modifiers: u8,
    pub screen_point: LayoutPoint,
    pub world_point: LayoutPoint,
    pub screen_delta: LayoutPoint,
    pub world_delta: LayoutPoint,
    pub bounds_before: Option<LayoutRect>,
    pub bounds_after: Option<LayoutRect>,
    pub marquee: Option<LayoutRect>,
}

pub(crate) fn canvas_interaction(
    target_id: WidgetId,
    target: &CanvasTarget,
    phase: CanvasInteractionPhase,
    point: LayoutPoint,
    delta: LayoutPoint,
    drag_start: Option<LayoutPoint>,
    layout: &LayoutSnapshot,
    viewports: &ViewportStateMap,
    input_kind: PointerKind,
    modifiers: u8,
) -> CanvasInteraction {
    let canvas_id = WidgetId::from_u128(target.canvas_id);
    let viewer_rect = layout
        .get_node_rect(canvas_id)
        .unwrap_or(LayoutRect::new(0.0, 0.0, 0.0, 0.0));
    let transform = viewports.transform(canvas_id).unwrap_or_default();
    let local = [point.x - viewer_rect.x(), point.y - viewer_rect.y()];
    let world = transform.screen_to_world(local);
    let world_point = LayoutPoint::new(world[0], world[1]);
    let world_delta = LayoutPoint::new(delta.x / transform.scale, delta.y / transform.scale);
    let (kind, bounds_before, bounds_after, marquee) = match &target.kind {
        CanvasTargetKind::Node { node_id, bounds } => {
            let before = rect(*bounds);
            let mut after = before;
            if matches!(phase, CanvasInteractionPhase::Update) {
                after.origin.x += world_delta.x;
                after.origin.y += world_delta.y;
                after.origin.x = snap(after.origin.x, target);
                after.origin.y = snap(after.origin.y, target);
            }
            (
                if matches!(phase, CanvasInteractionPhase::Activate) {
                    CanvasInteractionKind::SelectNode { node_id: *node_id }
                } else {
                    CanvasInteractionKind::MoveNode { node_id: *node_id }
                },
                Some(before),
                Some(after),
                None,
            )
        }
        CanvasTargetKind::ResizeHandle {
            node_id,
            handle,
            bounds,
        } => {
            let before = rect(*bounds);
            let after = resize(before, *handle, world_delta, target);
            (
                CanvasInteractionKind::ResizeNode {
                    node_id: *node_id,
                    handle: *handle,
                },
                Some(before),
                Some(after),
                None,
            )
        }
        CanvasTargetKind::Edge { edge_id, .. } => (
            CanvasInteractionKind::SelectEdge { edge_id: *edge_id },
            None,
            None,
            None,
        ),
        CanvasTargetKind::Marquee => {
            let marquee = drag_start.map(|start| {
                let start_local = [start.x - viewer_rect.x(), start.y - viewer_rect.y()];
                let start_world = transform.screen_to_world(start_local);
                normalized_rect(
                    LayoutPoint::new(start_world[0], start_world[1]),
                    world_point,
                )
            });
            (CanvasInteractionKind::Marquee, None, None, marquee)
        }
    };
    CanvasInteraction {
        canvas_id,
        target_id,
        kind,
        selection_policy: target.selection_policy,
        phase,
        input_kind,
        modifiers,
        screen_point: point,
        world_point,
        screen_delta: delta,
        world_delta,
        bounds_before,
        bounds_after,
        marquee,
    }
}

fn rect(bounds: [f32; 4]) -> LayoutRect {
    LayoutRect::new(bounds[0], bounds[1], bounds[2], bounds[3])
}

fn snap(value: f32, target: &CanvasTarget) -> f32 {
    let Some(spacing) = target.snap_spacing.filter(|spacing| *spacing > 0.0) else {
        return value;
    };
    let candidate = (value / spacing).round() * spacing;
    if target.snap_threshold <= 0.0 || (candidate - value).abs() <= target.snap_threshold {
        candidate
    } else {
        value
    }
}

fn resize(before: LayoutRect, handle: u8, delta: LayoutPoint, target: &CanvasTarget) -> LayoutRect {
    let mut left = before.x();
    let mut top = before.y();
    let mut right = before.right();
    let mut bottom = before.bottom();
    if matches!(handle, 0 | 6 | 7) {
        left = snap(left + delta.x, target).min(right - 1.0);
    }
    if matches!(handle, 2 | 3 | 4) {
        right = snap(right + delta.x, target).max(left + 1.0);
    }
    if matches!(handle, 0 | 1 | 2) {
        top = snap(top + delta.y, target).min(bottom - 1.0);
    }
    if matches!(handle, 4 | 5 | 6) {
        bottom = snap(bottom + delta.y, target).max(top + 1.0);
    }
    LayoutRect::new(left, top, right - left, bottom - top)
}

fn normalized_rect(first: LayoutPoint, second: LayoutPoint) -> LayoutRect {
    let left = first.x.min(second.x);
    let top = first.y.min(second.y);
    LayoutRect::new(
        left,
        top,
        (first.x - second.x).abs(),
        (first.y - second.y).abs(),
    )
}
