use super::{ControllerContext, InputController};
use crate::env::TextSelectionHandleKind;
use crate::event::{
    InputEvent, KeyCode, KeyEvent, PointerEvent, MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_SUPER,
};
use crate::ui::widgets::context_menu::TextContextMenuAction;
use crate::ui::widgets::text_input::{
    downcast_text_input_runtime_config, text_input_selection_handle_id,
    text_input_toolbar_button_id, DragStartBehavior,
};
use crate::ActionEnvelope;
use crate::ActionId;
use fission_ir::FlexDirection;
use fission_ir::{
    op::{self, decode_text_paragraph_style, LayoutOp, Op, TextAlign, TextParagraphStyle},
    semantics::{InputFormatter, MaxLengthEnforcement, TextCapitalization, TextInputType},
    Semantics, WidgetId,
};
use serde_json;
use unicode_segmentation::UnicodeSegmentation;

mod ime;
mod keyboard;
mod navigation;
mod support;

pub struct TextInputController;

impl InputController for TextInputController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => self.handle_key(ctx, key_code.clone(), *modifiers),
            InputEvent::Ime(ime) => self.handle_ime(ctx, ime),
            InputEvent::Pointer(PointerEvent::Down {
                point,
                button,
                modifiers,
                ..
            }) => {
                let hit =
                    crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point);

                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(sem) = &node.op {
                            if sem.role == fission_ir::semantics::Role::TextInput {
                                if let Some(hit_node_id) = hit {
                                    if let Some(action) =
                                        Self::toolbar_action_hit(ctx.ir, focused_id, hit_node_id)
                                    {
                                        return self.execute_toolbar_action(ctx, action);
                                    }
                                    if let Some(handle_kind) =
                                        Self::selection_handle_hit(ctx.ir, focused_id, hit_node_id)
                                    {
                                        let value = sem.value.as_deref().unwrap_or("").to_string();
                                        if matches!(button, crate::event::PointerButton::Primary) {
                                            ctx.interaction.pressed.clear();
                                            ctx.interaction.set_pressed(focused_id, true);
                                            ctx.interaction.last_down_point = Some(*point);
                                        }
                                        let state = ctx.text_edit.get_mut_or_default(focused_id);
                                        state.affordances.active_handle = Some(handle_kind);
                                        state.affordances.toolbar_visible = false;
                                        Self::sync_text_input_affordances(
                                            ctx, focused_id, sem, &value, false, None,
                                        );
                                        return true;
                                    }
                                }

                                if matches!(button, crate::event::PointerButton::Secondary) {
                                    let value = sem.value.as_deref().unwrap_or("").to_string();
                                    let wrapper_anchor =
                                        Self::input_wrapper_geometry(ctx, focused_id).map(|geom| {
                                            fission_layout::LayoutPoint::new(
                                                (point.x - geom.rect.origin.x).max(0.0),
                                                (point.y - geom.rect.origin.y).max(0.0),
                                            )
                                        });
                                    Self::sync_text_input_affordances(
                                        ctx,
                                        focused_id,
                                        sem,
                                        &value,
                                        true,
                                        wrapper_anchor,
                                    );
                                    return true;
                                }
                            }
                        }
                    }
                }

                // Only keep handling pointer-down inside the already-focused input
                // if the hit test still resolves into that subtree. Otherwise we
                // must fall through so Runtime can move focus to a different
                // widget instead of swallowing the click.
                let effective_focused = if let Some(focused_id) = ctx.interaction.focused {
                    let mut walk = hit;
                    let mut belongs_to_focused = false;
                    while let Some(nid) = walk {
                        if nid == focused_id {
                            belongs_to_focused = true;
                            break;
                        }
                        walk = ctx.ir.nodes.get(&nid).and_then(|n| n.parent);
                    }
                    if belongs_to_focused {
                        Some(focused_id)
                    } else {
                        if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                            if let Op::Semantics(sem) = &node.op {
                                if sem.role == fission_ir::semantics::Role::TextInput {
                                    let current_value = sem.value.as_deref().unwrap_or("");
                                    let _ = Self::dispatch_action_for_trigger(
                                        ctx,
                                        sem,
                                        focused_id,
                                        fission_ir::semantics::ActionTrigger::TapOutside,
                                        Some(
                                            serde_json::to_vec(&current_value.to_string()).unwrap(),
                                        ),
                                    );
                                }
                            }
                        }
                        Self::clear_text_input_affordances(ctx, focused_id);
                        None
                    }
                } else {
                    // If nothing is focused, try to find the TextInput under the
                    // click point and focus + place the caret in one step.
                    hit.and_then(|hit| {
                        let mut walk = Some(hit);
                        while let Some(nid) = walk {
                            if let Some(node) = ctx.ir.nodes.get(&nid) {
                                if let Op::Semantics(s) = &node.op {
                                    if s.focusable
                                        && s.role == fission_ir::semantics::Role::TextInput
                                    {
                                        ctx.interaction.set_focused(Some(nid));
                                        return Some(nid);
                                    }
                                }
                                walk = node.parent;
                            } else {
                                break;
                            }
                        }
                        None
                    })
                };
                if let Some(focused_id) = effective_focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(sem) = &node.op {
                            if sem.role == fission_ir::semantics::Role::TextInput {
                                // Only handle pointer-down as a caret/selection update when the
                                // pointer is inside the currently focused TextInput.
                                //
                                // Otherwise, allow the generic focus logic in `Runtime::handle_input`
                                // to run so clicks can move focus to other widgets (including other
                                // TextInputs, buttons, etc).
                                //
                                // The geometry rect is in layout coordinates (no scroll offset applied).
                                // We need to adjust the rect by ancestor scroll offsets to compare
                                // against the screen-coordinate click point.
                                // The focused_id is a Semantics node which may not have
                                // layout geometry.  Walk to its first child or parent
                                // that has geometry for the containment check.
                                let geom_id = std::iter::successors(Some(focused_id), |id| {
                                    ctx.ir
                                        .nodes
                                        .get(id)
                                        .and_then(|n| n.children.first().copied())
                                })
                                .find(|id| ctx.layout.get_node_geometry(*id).is_some())
                                .or_else(|| {
                                    let mut w =
                                        ctx.ir.nodes.get(&focused_id).and_then(|n| n.parent);
                                    while let Some(pid) = w {
                                        if ctx.layout.get_node_geometry(pid).is_some() {
                                            return Some(pid);
                                        }
                                        w = ctx.ir.nodes.get(&pid).and_then(|n| n.parent);
                                    }
                                    None
                                });
                                if let Some(geom) =
                                    geom_id.and_then(|id| ctx.layout.get_node_geometry(id))
                                {
                                    let mut scroll_adj_y = 0.0f32;
                                    let mut scroll_adj_x = 0.0f32;
                                    let mut walk_id =
                                        ctx.ir.nodes.get(&focused_id).and_then(|n| n.parent);
                                    while let Some(pid) = walk_id {
                                        if let Some(pnode) = ctx.ir.nodes.get(&pid) {
                                            if let Op::Layout(LayoutOp::Scroll {
                                                direction, ..
                                            }) = &pnode.op
                                            {
                                                let poff = ctx.scroll.get_offset(pid);
                                                match direction {
                                                    FlexDirection::Row => scroll_adj_x += poff,
                                                    FlexDirection::Column => scroll_adj_y += poff,
                                                }
                                            }
                                            walk_id = pnode.parent;
                                        } else {
                                            break;
                                        }
                                    }
                                    let visual_rect = fission_layout::LayoutRect::new(
                                        geom.rect.origin.x - scroll_adj_x,
                                        geom.rect.origin.y - scroll_adj_y,
                                        geom.rect.size.width,
                                        geom.rect.size.height,
                                    );
                                    // Skip containment check — the focus logic already verified
                                    // the click is on this TextInput
                                    let _ = visual_rect;
                                }
                                let scroll_result = Self::find_scroll_container_and_text_op(
                                    ctx.ir,
                                    focused_id,
                                    sem.multiline,
                                );
                                if let Some((scroll_id, _text_op_node_id, scroll_direction)) =
                                    scroll_result
                                {
                                    if let Some(scroll_geom) =
                                        ctx.layout.get_node_geometry(scroll_id)
                                    {
                                        if matches!(button, crate::event::PointerButton::Primary) {
                                            ctx.interaction.pressed.clear();
                                            ctx.interaction.set_pressed(focused_id, true);
                                            ctx.interaction.last_down_point = Some(*point);
                                        }
                                        let value = sem.value.as_deref().unwrap_or("");
                                        let display_value =
                                            Self::display_value_for_metrics(ctx, focused_id, value);
                                        let metric_text = if sem.masked {
                                            Self::mask_text_for_metrics(&display_value)
                                        } else {
                                            display_value.clone()
                                        };

                                        let caret = if let Some(measurer) = ctx.measurer {
                                            let local_point = Self::text_local_point_from_screen(
                                                ctx,
                                                scroll_id,
                                                scroll_direction,
                                                scroll_geom,
                                                *point,
                                            );

                                            let masked_caret = Self::hit_test_text(
                                                measurer,
                                                ctx.ir,
                                                focused_id,
                                                sem.masked,
                                                &metric_text,
                                                scroll_geom,
                                                local_point.x,
                                                local_point.y,
                                            );
                                            if sem.masked {
                                                Self::source_byte_offset_from_masked(
                                                    &display_value,
                                                    &metric_text,
                                                    masked_caret,
                                                )
                                            } else {
                                                masked_caret
                                            }
                                        } else {
                                            let font_size =
                                                Self::extract_font_size(ctx.ir, focused_id)
                                                    .unwrap_or(13.0);
                                            Self::caret_from_point_in_text_fallback(
                                                &display_value,
                                                font_size,
                                                scroll_geom.rect.origin.x,
                                                scroll_geom.rect.size.width,
                                                scroll_geom.content_size.width,
                                                ctx.scroll.get_offset(scroll_id),
                                                point.x,
                                            )
                                        };
                                        let anchor = {
                                            let st = ctx.text_edit.get_mut_or_default(focused_id);
                                            st.caret = caret;
                                            if !Self::has_shift(*modifiers) {
                                                st.anchor = caret;
                                            }
                                            st.anchor
                                        };
                                        Self::dispatch_cursor_change(
                                            ctx, sem, focused_id, caret, anchor,
                                        );
                                        Self::sync_text_input_affordances(
                                            ctx, focused_id, sem, value, false, None,
                                        );
                                    }
                                }
                                return true;
                            }
                        }
                    }
                }

                false
            }
            InputEvent::Pointer(PointerEvent::Move { point, .. }) => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(sem) = &node.op {
                            if sem.role == fission_ir::semantics::Role::TextInput {
                                let active_handle = ctx
                                    .text_edit
                                    .states
                                    .get(&focused_id)
                                    .and_then(|state| state.affordances.active_handle);
                                if let Some(active_handle) = active_handle {
                                    if let Some((scroll_id, _text_op_node_id, scroll_direction)) =
                                        Self::find_scroll_container_and_text_op(
                                            ctx.ir,
                                            focused_id,
                                            sem.multiline,
                                        )
                                    {
                                        if let Some(scroll_geom) =
                                            ctx.layout.get_node_geometry(scroll_id)
                                        {
                                            let value = sem.value.as_deref().unwrap_or("");
                                            let display_value = Self::display_value_for_metrics(
                                                ctx, focused_id, value,
                                            );
                                            let metric_text = if sem.masked {
                                                Self::mask_text_for_metrics(&display_value)
                                            } else {
                                                display_value.clone()
                                            };
                                            let new_caret = if let Some(measurer) = ctx.measurer {
                                                let local_point =
                                                    Self::text_local_point_from_screen(
                                                        ctx,
                                                        scroll_id,
                                                        scroll_direction,
                                                        scroll_geom,
                                                        *point,
                                                    );
                                                let masked_caret = Self::hit_test_text(
                                                    measurer,
                                                    ctx.ir,
                                                    focused_id,
                                                    sem.masked,
                                                    &metric_text,
                                                    scroll_geom,
                                                    local_point.x,
                                                    local_point.y,
                                                );
                                                if sem.masked {
                                                    Self::source_byte_offset_from_masked(
                                                        &display_value,
                                                        &metric_text,
                                                        masked_caret,
                                                    )
                                                } else {
                                                    masked_caret
                                                }
                                            } else {
                                                0
                                            };
                                            let (caret, anchor) = {
                                                let st =
                                                    ctx.text_edit.get_mut_or_default(focused_id);
                                                match active_handle {
                                                    TextSelectionHandleKind::Caret => {
                                                        st.caret = new_caret;
                                                        st.anchor = new_caret;
                                                    }
                                                    TextSelectionHandleKind::Start => {
                                                        if st.caret <= st.anchor {
                                                            st.caret = new_caret;
                                                        } else {
                                                            st.anchor = new_caret;
                                                        }
                                                    }
                                                    TextSelectionHandleKind::End => {
                                                        if st.caret >= st.anchor {
                                                            st.caret = new_caret;
                                                        } else {
                                                            st.anchor = new_caret;
                                                        }
                                                    }
                                                }
                                                (st.caret, st.anchor)
                                            };
                                            Self::auto_scroll_textinput(ctx, focused_id);
                                            Self::dispatch_cursor_change(
                                                ctx, sem, focused_id, caret, anchor,
                                            );
                                            Self::sync_text_input_affordances(
                                                ctx, focused_id, sem, value, false, None,
                                            );
                                        }
                                    }
                                    return true;
                                }

                                if ctx.interaction.is_pressed(focused_id) {
                                    let moved_enough =
                                        match Self::drag_start_behavior(ctx, focused_id) {
                                            DragStartBehavior::Down => true,
                                            DragStartBehavior::Start => {
                                                let mut moved_enough = true;
                                                if let Some(start) = ctx.interaction.last_down_point
                                                {
                                                    let dx = point.x - start.x;
                                                    let dy = point.y - start.y;
                                                    if dx * dx + dy * dy < 4.0 {
                                                        moved_enough = false;
                                                    }
                                                }
                                                moved_enough
                                            }
                                        };
                                    if moved_enough {
                                        if let Some((
                                            scroll_id,
                                            _text_op_node_id,
                                            scroll_direction,
                                        )) = Self::find_scroll_container_and_text_op(
                                            ctx.ir,
                                            focused_id,
                                            sem.multiline,
                                        ) {
                                            if let Some(scroll_geom) =
                                                ctx.layout.get_node_geometry(scroll_id)
                                            {
                                                let value = sem.value.as_deref().unwrap_or("");
                                                let display_value = Self::display_value_for_metrics(
                                                    ctx, focused_id, value,
                                                );
                                                let metric_text = if sem.masked {
                                                    Self::mask_text_for_metrics(&display_value)
                                                } else {
                                                    display_value.clone()
                                                };
                                                let new_caret = if let Some(measurer) = ctx.measurer
                                                {
                                                    let local_point =
                                                        Self::text_local_point_from_screen(
                                                            ctx,
                                                            scroll_id,
                                                            scroll_direction,
                                                            scroll_geom,
                                                            *point,
                                                        );

                                                    let masked_caret = Self::hit_test_text(
                                                        measurer,
                                                        ctx.ir,
                                                        focused_id,
                                                        sem.masked,
                                                        &metric_text,
                                                        scroll_geom,
                                                        local_point.x,
                                                        local_point.y,
                                                    );
                                                    if sem.masked {
                                                        Self::source_byte_offset_from_masked(
                                                            &display_value,
                                                            &metric_text,
                                                            masked_caret,
                                                        )
                                                    } else {
                                                        masked_caret
                                                    }
                                                } else {
                                                    let font_size =
                                                        Self::extract_font_size(ctx.ir, focused_id)
                                                            .unwrap_or(13.0);
                                                    Self::caret_from_point_in_text_fallback(
                                                        &display_value,
                                                        font_size,
                                                        scroll_geom.rect.origin.x,
                                                        scroll_geom.rect.size.width,
                                                        scroll_geom.content_size.width,
                                                        ctx.scroll.get_offset(scroll_id),
                                                        point.x,
                                                    )
                                                };
                                                let st =
                                                    ctx.text_edit.get_mut_or_default(focused_id);
                                                st.caret = new_caret;
                                                let current_anchor = st.anchor;
                                                Self::auto_scroll_textinput(ctx, focused_id);
                                                Self::dispatch_cursor_change(
                                                    ctx,
                                                    sem,
                                                    focused_id,
                                                    new_caret,
                                                    current_anchor,
                                                );
                                                Self::sync_text_input_affordances(
                                                    ctx, focused_id, sem, value, false, None,
                                                );
                                            }
                                        }
                                    }
                                }
                                return true;
                            }
                        }
                    }
                }

                false
            }
            InputEvent::Pointer(PointerEvent::Up { point, button, .. }) => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(sem) = &node.op {
                            if sem.role == fission_ir::semantics::Role::TextInput {
                                let value = sem.value.as_deref().unwrap_or("").to_string();
                                let toolbar_anchor = Self::input_wrapper_geometry(ctx, focused_id)
                                    .map(|geom| {
                                        fission_layout::LayoutPoint::new(
                                            (point.x - geom.rect.origin.x).max(0.0),
                                            (point.y - geom.rect.origin.y).max(0.0),
                                        )
                                    });
                                let show_toolbar =
                                    matches!(button, crate::event::PointerButton::Secondary)
                                        || ctx
                                            .text_edit
                                            .states
                                            .get(&focused_id)
                                            .map(|state| state.caret != state.anchor)
                                            .unwrap_or(false);
                                if let Some(state) = ctx.text_edit.states.get_mut(&focused_id) {
                                    state.affordances.active_handle = None;
                                    state.affordances.magnifier_visible = false;
                                }
                                Self::sync_text_input_affordances(
                                    ctx,
                                    focused_id,
                                    sem,
                                    &value,
                                    show_toolbar,
                                    if show_toolbar { toolbar_anchor } else { None },
                                );
                                return true;
                            }
                        }
                    }
                }

                false
            }
            _ => false,
        }
    }
}

// This pub fn is no longer needed since Controller uses measurer directly in handle_event
// But other parts of code might still call it, so keep it.
pub fn caret_from_point_in_text(
    measurer: Option<&std::sync::Arc<dyn fission_layout::TextMeasurer>>,
    value: &str,
    font_size: f32,
    viewport_x: f32,
    viewport_w: f32,
    content_w: f32,
    scroll_offset: f32,
    point_x: f32,
) -> usize {
    let local_x = (point_x - viewport_x) + scroll_offset;
    if local_x <= 0.0 {
        return 0;
    }
    let max_x = content_w.max(viewport_w);
    if local_x >= max_x {
        return value.len();
    }

    if let Some(measurer) = measurer {
        // This function is for single line mostly. measurer.hit_test is better.
        // Single-line hit-testing should not wrap text to the viewport width.
        measurer.hit_test(value, font_size, None, local_x, 0.0)
    } else {
        TextInputController::caret_from_point_in_text_fallback(
            value,
            font_size,
            viewport_x,
            viewport_w,
            content_w,
            scroll_offset,
            point_x,
        )
    }
}
