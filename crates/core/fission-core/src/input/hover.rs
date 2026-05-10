use crate::event::{InputEvent, PointerEvent};
use crate::hit_test::hit_test_with_scroll;
use crate::input::{ControllerContext, InputController};
use crate::{ActionEnvelope, ActionId, ActionInput};
use fission_ir::op::{PaintOp, RichTextAnnotation};
use fission_ir::semantics::{ActionTrigger, MouseCursor};
use fission_ir::{NodeId, Op};
use fission_layout::{LayoutPoint, LayoutRect};

type ResolvedRichTextAnnotation = (NodeId, RichTextAnnotation);

pub struct HoverController;

impl HoverController {
    pub fn clear(ctx: &mut ControllerContext, point: Option<LayoutPoint>) -> bool {
        Self::apply_hover_path(ctx, Vec::new(), point)
    }

    fn hover_path_at_point(ctx: &ControllerContext, point: LayoutPoint) -> Vec<NodeId> {
        let Some(hit_node_id) = hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, point) else {
            return Vec::new();
        };

        let mut path = Vec::new();
        let mut current = Some(hit_node_id);
        while let Some(node_id) = current {
            path.push(node_id);
            current = ctx.ir.nodes.get(&node_id).and_then(|node| node.parent);
        }
        path
    }

    fn apply_hover_path(
        ctx: &mut ControllerContext,
        next_path: Vec<NodeId>,
        point: Option<LayoutPoint>,
    ) -> bool {
        let previous_path = ctx.interaction.hover_path.clone();
        let previous_annotation = ctx.interaction.hovered_rich_text_annotation().cloned();
        let next_annotation =
            point.and_then(|point| resolve_rich_text_annotation_at_point(ctx, &next_path, point));
        let common_tail_len = shared_tail_len(&previous_path, &next_path);
        let exited = &previous_path[..previous_path.len().saturating_sub(common_tail_len)];
        let entered = &next_path[..next_path.len().saturating_sub(common_tail_len)];

        for node_id in exited {
            ctx.interaction.set_hovered(*node_id, false);
        }
        for node_id in entered {
            ctx.interaction.set_hovered(*node_id, true);
        }

        for node_id in exited {
            dispatch_hover_actions(ctx, *node_id, ActionTrigger::HoverExit, point);
        }
        for node_id in entered.iter().rev() {
            dispatch_hover_actions(ctx, *node_id, ActionTrigger::HoverEnter, point);
        }

        if previous_annotation
            .as_ref()
            .map(|annotation| (&annotation.node_id, &annotation.annotation))
            != next_annotation
                .as_ref()
                .map(|(node_id, annotation)| (node_id, annotation))
        {
            if let Some(previous) = &previous_annotation {
                dispatch_annotation_actions(
                    ctx,
                    previous.node_id,
                    &previous.annotation,
                    ActionTrigger::HoverExit,
                    point,
                );
            }
            if let Some((node_id, annotation)) = &next_annotation {
                dispatch_annotation_actions(
                    ctx,
                    *node_id,
                    annotation,
                    ActionTrigger::HoverEnter,
                    point,
                );
            }
        }

        let next_cursor = resolve_cursor(ctx, &next_path, next_annotation.as_ref());
        let changed = previous_path != next_path
            || previous_annotation
                .as_ref()
                .map(|annotation| (&annotation.node_id, &annotation.annotation))
                != next_annotation
                    .as_ref()
                    .map(|(node_id, annotation)| (node_id, annotation))
            || ctx.interaction.cursor != next_cursor;
        ctx.interaction.set_hover_path(next_path);
        ctx.interaction
            .set_hovered_rich_text_annotation(next_annotation.map(|(node_id, annotation)| {
                crate::env::HoveredRichTextAnnotation {
                    node_id,
                    annotation,
                }
            }));
        ctx.interaction.set_cursor(next_cursor);
        changed
    }
}

impl InputController for HoverController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Pointer(PointerEvent::Down { point, .. })
            | InputEvent::Pointer(PointerEvent::Up { point, .. })
            | InputEvent::Pointer(PointerEvent::Move { point, .. })
            | InputEvent::Pointer(PointerEvent::Scroll { point, .. }) => {
                let next_path = Self::hover_path_at_point(ctx, *point);
                let _ = Self::apply_hover_path(ctx, next_path, Some(*point));
            }
            _ => {}
        }
        false
    }
}

fn shared_tail_len(previous_path: &[NodeId], next_path: &[NodeId]) -> usize {
    previous_path
        .iter()
        .rev()
        .zip(next_path.iter().rev())
        .take_while(|(previous, next)| previous == next)
        .count()
}

fn resolve_cursor(
    ctx: &ControllerContext,
    hover_path: &[NodeId],
    rich_text_annotation: Option<&ResolvedRichTextAnnotation>,
) -> MouseCursor {
    if let Some((_, annotation)) = rich_text_annotation {
        if let Some(cursor) = annotation.mouse_cursor.map(map_rich_text_cursor) {
            return cursor;
        }
    }

    for node_id in hover_path {
        let Some(node) = ctx.ir.nodes.get(node_id) else {
            continue;
        };
        let Op::Semantics(semantics) = &node.op else {
            continue;
        };
        if let Some(cursor) = semantics
            .actions
            .entries
            .iter()
            .find_map(|entry| entry.as_hover_cursor())
        {
            return cursor;
        }
    }

    MouseCursor::Default
}

fn map_rich_text_cursor(cursor: fission_ir::op::MouseCursor) -> MouseCursor {
    match cursor {
        fission_ir::op::MouseCursor::Basic => MouseCursor::Default,
        fission_ir::op::MouseCursor::Pointer => MouseCursor::Pointer,
        fission_ir::op::MouseCursor::Text => MouseCursor::Text,
    }
}

pub(crate) fn resolve_rich_text_annotation_at_point(
    ctx: &ControllerContext,
    hover_path: &[NodeId],
    point: LayoutPoint,
) -> Option<ResolvedRichTextAnnotation> {
    let measurer = ctx.measurer?;

    for node_id in hover_path {
        let Some(any_annotations) = ctx.ir.custom_render_objects.get(node_id) else {
            continue;
        };
        let Some(annotations) = any_annotations.downcast_ref::<Vec<RichTextAnnotation>>() else {
            continue;
        };
        let Some(node) = ctx.ir.nodes.get(node_id) else {
            continue;
        };
        let Op::Paint(PaintOp::DrawRichText {
            runs,
            wrap,
            paragraph_style,
            ..
        }) = &node.op
        else {
            continue;
        };
        let Some(rect) = visual_rect_for_node(ctx, *node_id) else {
            continue;
        };
        let local_x = point.x - rect.origin.x;
        let local_y = point.y - rect.origin.y;
        let available_width = if *wrap && rect.width() > 0.0 {
            Some(rect.width())
        } else {
            None
        };

        if let Some(annotation) = measurer.resolve_rich_text_annotation_at_point(
            runs,
            available_width,
            local_x,
            local_y,
            paragraph_style.unwrap_or_default(),
            annotations,
        ) {
            return Some((*node_id, annotation));
        }
    }

    None
}

fn visual_rect_for_node(ctx: &ControllerContext, node_id: NodeId) -> Option<LayoutRect> {
    let mut rect = ctx.layout.get_node_rect(node_id)?;
    let mut current = ctx.ir.nodes.get(&node_id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        let Some(parent) = ctx.ir.nodes.get(&parent_id) else {
            break;
        };
        if let Op::Layout(fission_ir::LayoutOp::Scroll { direction, .. }) = &parent.op {
            let offset = ctx.scroll.get_offset(parent_id);
            match direction {
                fission_ir::FlexDirection::Row => rect.origin.x -= offset,
                fission_ir::FlexDirection::Column => rect.origin.y -= offset,
            }
        }
        current = parent.parent;
    }
    Some(rect)
}

fn dispatch_hover_actions(
    ctx: &mut ControllerContext,
    node_id: NodeId,
    trigger: ActionTrigger,
    point: Option<LayoutPoint>,
) {
    let Some(node) = ctx.ir.nodes.get(&node_id) else {
        return;
    };
    let Op::Semantics(semantics) = &node.op else {
        return;
    };

    for entry in semantics
        .actions
        .entries
        .iter()
        .filter(|entry| entry.trigger == trigger)
    {
        let Some(payload) = &entry.payload_data else {
            continue;
        };
        let input = point.map(pointer_input).unwrap_or(ActionInput::None);
        ctx.dispatched_actions.push((
            node_id,
            ActionEnvelope {
                id: ActionId::from_u128(entry.action_id),
                payload: payload.clone(),
            },
            input,
        ));
    }
}

fn dispatch_annotation_actions(
    ctx: &mut ControllerContext,
    node_id: NodeId,
    annotation: &RichTextAnnotation,
    trigger: ActionTrigger,
    point: Option<LayoutPoint>,
) {
    for entry in annotation
        .actions
        .iter()
        .filter(|entry| entry.trigger == trigger)
    {
        let Some(payload) = &entry.payload_data else {
            continue;
        };
        let input = point.map(pointer_input).unwrap_or(ActionInput::None);
        ctx.dispatched_actions.push((
            node_id,
            ActionEnvelope {
                id: ActionId::from_u128(entry.action_id),
                payload: payload.clone(),
            },
            input,
        ));
    }
}

fn pointer_input(point: LayoutPoint) -> ActionInput {
    ActionInput::Pointer {
        x: point.x,
        y: point.y,
        delta_x: 0.0,
        delta_y: 0.0,
    }
}
