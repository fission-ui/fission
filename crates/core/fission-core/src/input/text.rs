use super::{ControllerContext, InputController};
use crate::env::TextSelectionHandleKind;
use crate::event::{
    EditingCommand, InputEvent, KeyCode, KeyEvent, PointerEvent, MOD_ALT, MOD_CTRL, MOD_SHIFT,
    MOD_SUPER,
};
use crate::ui::widgets::context_menu::TextContextMenuAction;
use crate::ui::widgets::text_input::{
    downcast_text_input_runtime_config, text_input_selection_handle_id,
    text_input_toolbar_button_id, DragStartBehavior, TextScrollPolicy,
};
use crate::ActionEnvelope;
use crate::ActionId;
use fission_ir::FlexDirection;
use fission_ir::{
    op::{self, decode_text_paragraph_style, LayoutOp, Op, TextAlign, TextParagraphStyle},
    semantics::InputFormatter,
    Semantics, WidgetId,
};
use serde_json;
use unicode_segmentation::UnicodeSegmentation;

pub struct TextInputController;

impl InputController for TextInputController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => self.handle_key(ctx, key_code.clone(), *modifiers),
            InputEvent::Keyboard(KeyEvent::DownWithText {
                key_code,
                modifiers,
                text,
            }) => self.handle_key_with_produced_text(ctx, key_code.clone(), *modifiers, text),
            InputEvent::Editing(command) => self.handle_editing_command(ctx, command),
            InputEvent::TextEdit(command) => self.handle_text_edit_command(ctx, command.clone()),
            InputEvent::Ime(ime) => self.handle_ime(ctx, ime),
            InputEvent::Pointer(PointerEvent::Down {
                point,
                button,
                modifiers,
                ..
            }) => {
                let hit = crate::hit_test::hit_test_with_viewports(
                    ctx.ir,
                    ctx.layout,
                    ctx.scroll,
                    ctx.viewport,
                    *point,
                );

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
                                        let semantic_value =
                                            s.value.as_deref().unwrap_or_default().to_string();
                                        let select_all = Self::runtime_config(ctx, nid)
                                            .is_some_and(|config| config.select_all_on_focus);
                                        ctx.interaction.set_focused(Some(nid));
                                        if select_all {
                                            Self::sync_runtime_state(
                                                ctx,
                                                nid,
                                                semantic_value.as_str(),
                                            );
                                            let state = ctx.text_edit.get_mut_or_default(nid);
                                            state.anchor = 0;
                                            state.caret = state.buffer.len_bytes();
                                        }
                                        let value = ctx
                                            .text_edit
                                            .get(nid)
                                            .map(|state| state.editing_value())
                                            .unwrap_or_else(|| {
                                                crate::TextEditingValue::from_text(
                                                    semantic_value.clone(),
                                                )
                                            });
                                        if let Some((envelope, input)) =
                                            crate::input::prepare_scoped_text_session_action(
                                                ctx.ir,
                                                s,
                                                nid,
                                                fission_ir::semantics::ActionTrigger::Focus,
                                                value,
                                                crate::TextEditSource::Pointer,
                                                crate::TextEditPhase::Focused,
                                            )
                                        {
                                            ctx.dispatched_actions.push((nid, envelope, input));
                                        }
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
                                if let Some((scroll_id, text_op_node_id, scroll_direction)) =
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
                                                ctx.layout,
                                                ctx.ir,
                                                focused_id,
                                                text_op_node_id,
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
                                    if let Some((scroll_id, text_op_node_id, scroll_direction)) =
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
                                                    ctx.layout,
                                                    ctx.ir,
                                                    focused_id,
                                                    text_op_node_id,
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
                                            text_op_node_id,
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
                                                        ctx.layout,
                                                        ctx.ir,
                                                        focused_id,
                                                        text_op_node_id,
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

impl TextInputController {
    fn handle_text_edit_command(
        &mut self,
        ctx: &mut ControllerContext,
        command: crate::TextEditCommand,
    ) -> bool {
        let Some(focused_id) = ctx.interaction.focused else {
            return false;
        };
        self.handle_text_edit_command_for(ctx, focused_id, command)
    }

    pub(crate) fn handle_text_edit_command_for(
        &mut self,
        ctx: &mut ControllerContext,
        focused_id: WidgetId,
        command: crate::TextEditCommand,
    ) -> bool {
        let Some(semantics) = Self::text_input_semantics(ctx, focused_id) else {
            return false;
        };
        if semantics.disabled || semantics.read_only {
            return true;
        }
        Self::sync_runtime_state(
            ctx,
            focused_id,
            semantics.value.as_deref().unwrap_or_default(),
        );
        let runtime = Self::runtime_config(ctx, focused_id);
        let old_value = ctx.text_edit.get_mut_or_default(focused_id).editing_value();
        let mut formatters = semantics.input_formatters.clone();
        if !semantics.multiline && !formatters.contains(&InputFormatter::SingleLine) {
            formatters.push(InputFormatter::SingleLine);
        }
        let pipeline = crate::TextEditPipeline {
            formatters,
            custom_formatters: runtime
                .map(|config| config.custom_input_formatters)
                .unwrap_or_default(),
            max_length: semantics.max_length,
            max_length_enforcement: semantics.max_length_enforcement,
        };
        let Ok(result) = pipeline.apply(&old_value, command) else {
            return true;
        };
        if matches!(result.phase, crate::TextEditPhase::Submitted) {
            Self::dispatch_action_for_trigger(
                ctx,
                &semantics,
                focused_id,
                fission_ir::semantics::ActionTrigger::Submit,
                None,
            );
            return true;
        }
        if matches!(result.phase, crate::TextEditPhase::EditingCompleted) {
            Self::dispatch_action_for_trigger(
                ctx,
                &semantics,
                focused_id,
                fission_ir::semantics::ActionTrigger::EditingComplete,
                None,
            );
            return true;
        }
        let source = result.source;
        let mut caret = result.new_value.selection.extent.utf8_offset();
        let mut anchor = result.new_value.selection.base.utf8_offset();
        match result.phase {
            crate::TextEditPhase::Selection => {
                let state = ctx.text_edit.get_mut_or_default(focused_id);
                state.caret = caret;
                state.anchor = anchor;
            }
            crate::TextEditPhase::CompositionCancelled => {
                let state = ctx.text_edit.get_mut_or_default(focused_id);
                state.clear_preedit();
                // The current IR may contain the projected composing value from
                // the preceding frame. Keep the restored base authoritative
                // until the next declarative rebuild supplies the model value.
                state.pending_model_sync = true;
                state.last_model_text = semantics.value.clone().unwrap_or_default();
                caret = state.caret;
                anchor = state.anchor;
            }
            crate::TextEditPhase::CompositionStarted | crate::TextEditPhase::CompositionUpdated => {
                let state = ctx.text_edit.get_mut_or_default(focused_id);
                state.sync_composing_value(result.new_value);
            }
            _ => {
                ctx.text_edit
                    .get_mut_or_default(focused_id)
                    .apply_editing_value(result.new_value.clone());
                self.dispatch_edit_result(ctx, &semantics, focused_id, result);
            }
        }
        Self::dispatch_cursor_change_from(ctx, &semantics, focused_id, caret, anchor, source);
        Self::auto_scroll_textinput(ctx, focused_id);
        true
    }

    fn handle_editing_command(
        &mut self,
        ctx: &mut ControllerContext,
        command: &EditingCommand,
    ) -> bool {
        let Some(focused_id) = ctx.interaction.focused else {
            return false;
        };
        let Some(semantics) = Self::text_input_semantics(ctx, focused_id) else {
            return false;
        };
        if semantics.disabled {
            return false;
        }

        let (value, caret, anchor) =
            Self::resolve_editing_value(ctx, focused_id, semantics.value.as_deref().unwrap_or(""));
        let caret = Self::clamp_caret_to_value(&value, caret);
        let anchor = Self::clamp_caret_to_value(&value, anchor);
        let selection = (caret != anchor).then_some((caret.min(anchor), caret.max(anchor)));

        match command {
            EditingCommand::Copy => {
                if let (Some((start, end)), Some(clipboard)) = (selection, ctx.clipboard) {
                    clipboard.set_text(&value[start..end]);
                }
            }
            EditingCommand::Cut => {
                if let Some((start, end)) = selection {
                    if let Some(clipboard) = ctx.clipboard {
                        clipboard.set_text(&value[start..end]);
                    }
                    if !semantics.read_only {
                        if let Some(result) = Self::apply_text_edit_transaction(
                            ctx,
                            &semantics,
                            focused_id,
                            start..end,
                            String::new(),
                            crate::TextEditSource::Clipboard,
                        ) {
                            let caret = result.new_value.selection.extent.utf8_offset();
                            let anchor = result.new_value.selection.base.utf8_offset();
                            self.dispatch_edit_result(ctx, &semantics, focused_id, result);
                            Self::dispatch_cursor_change(
                                ctx, &semantics, focused_id, caret, anchor,
                            );
                        }
                    }
                }
            }
            EditingCommand::Paste(text) => {
                if !semantics.read_only && !text.is_empty() {
                    let (start, end) = selection.unwrap_or((caret, caret));
                    if let Some(inserted) =
                        Self::prepare_inserted_text(&semantics, &value, start, end, text)
                    {
                        if let Some(result) = Self::apply_text_edit_transaction(
                            ctx,
                            &semantics,
                            focused_id,
                            start..end,
                            inserted,
                            crate::TextEditSource::Clipboard,
                        ) {
                            let caret = result.new_value.selection.extent.utf8_offset();
                            let anchor = result.new_value.selection.base.utf8_offset();
                            self.dispatch_edit_result(ctx, &semantics, focused_id, result);
                            Self::dispatch_cursor_change(
                                ctx, &semantics, focused_id, caret, anchor,
                            );
                        }
                    }
                }
            }
            EditingCommand::SelectAll => {
                let state = ctx.text_edit.get_mut_or_default(focused_id);
                state.caret = value.len();
                state.anchor = 0;
                state.clear_preedit();
                Self::dispatch_cursor_change(ctx, &semantics, focused_id, value.len(), 0);
            }
            EditingCommand::Undo | EditingCommand::Redo => {
                let edit = {
                    let state = ctx.text_edit.get_mut_or_default(focused_id);
                    match command {
                        EditingCommand::Undo => state.undo(),
                        EditingCommand::Redo => state.redo(),
                        _ => unreachable!(),
                    }
                };
                if let Some((next, next_caret, next_anchor)) = edit {
                    self.dispatch_change(ctx, &semantics, focused_id, next);
                    Self::dispatch_cursor_change(
                        ctx,
                        &semantics,
                        focused_id,
                        next_caret,
                        next_anchor,
                    );
                }
            }
        }

        let displayed_value = ctx
            .text_edit
            .get(focused_id)
            .map(|state| state.committed_text().to_owned())
            .unwrap_or(value);
        Self::sync_text_input_affordances(
            ctx,
            focused_id,
            &semantics,
            displayed_value.as_str(),
            false,
            None,
        );
        true
    }

    fn text_input_semantics(ctx: &ControllerContext, focused_id: WidgetId) -> Option<Semantics> {
        let mut current_id = Some(focused_id);
        while let Some(node_id) = current_id {
            let node = ctx.ir.nodes.get(&node_id)?;
            if let Op::Semantics(semantics) = &node.op {
                if semantics.role == fission_ir::semantics::Role::TextInput {
                    return Some(semantics.clone());
                }
            }
            current_id = node.parent;
        }
        None
    }

    fn handle_key_with_produced_text(
        &mut self,
        ctx: &mut ControllerContext,
        key_code: KeyCode,
        modifiers: u8,
        text: &str,
    ) -> bool {
        let shortcut = ctx.editing_convention.has_primary_shortcut(modifiers)
            && !ctx.editing_convention.is_alt_gr(modifiers);
        let text_key = matches!(key_code, KeyCode::Char(_) | KeyCode::Space);
        if shortcut || !text_key || text.is_empty() {
            return self.handle_key(ctx, key_code, modifiers);
        }
        let Some(focused_id) = ctx.interaction.focused else {
            return false;
        };
        let Some(semantics) = Self::text_input_semantics(ctx, focused_id) else {
            return false;
        };
        if semantics.disabled || semantics.read_only {
            return true;
        }
        Self::sync_runtime_state(
            ctx,
            focused_id,
            semantics.value.as_deref().unwrap_or_default(),
        );
        let value = ctx.text_edit.get_mut_or_default(focused_id).editing_value();
        self.handle_text_edit_command(
            ctx,
            crate::TextEditCommand::Replace {
                range: value.selection_range(),
                text: text.to_string(),
                source: crate::TextEditSource::Keyboard,
            },
        )
    }

    fn handle_key(
        &mut self,
        ctx: &mut ControllerContext,
        key_code: KeyCode,
        modifiers: u8,
    ) -> bool {
        let focused_id = if let Some(id) = ctx.interaction.focused {
            id
        } else {
            return false;
        };

        let mut semantics_node = None;
        let mut current_id = Some(focused_id);
        while let Some(node_id) = current_id {
            if let Some(node) = ctx.ir.nodes.get(&node_id) {
                if let Op::Semantics(s) = &node.op {
                    if s.role == fission_ir::semantics::Role::TextInput {
                        semantics_node = Some(s);
                        break;
                    }
                }
                current_id = node.parent;
            } else {
                break;
            }
        }

        let semantics = if let Some(s) = semantics_node {
            s
        } else {
            return false;
        };

        let (value, mut caret, mut anchor) =
            Self::resolve_editing_value(ctx, focused_id, semantics.value.as_deref().unwrap_or(""));
        if let Some(st) = ctx.text_edit.states.get_mut(&focused_id) {
            st.clear_preedit();
        }

        caret = Self::clamp_caret_to_value(&value, caret);
        anchor = Self::clamp_caret_to_value(&value, anchor);

        let sel = if caret != anchor {
            Some((caret.min(anchor), caret.max(anchor)))
        } else {
            None
        };

        // Logic for state changes
        let mut next_caret = caret;
        let mut next_anchor = anchor;
        let mut next_edit: Option<(std::ops::Range<usize>, String)> = None;
        let mut handled = false;

        let read_only = semantics.read_only;
        let disabled = semantics.disabled;
        let convention = ctx.editing_convention;
        let is_apple = convention.is_apple();
        let shift = Self::has_shift(modifiers);
        let primary_shortcut =
            convention.has_primary_shortcut(modifiers) && !convention.is_alt_gr(modifiers);
        let word_modifier = convention.has_word_modifier(modifiers);

        if disabled {
            return false;
        }

        match key_code {
            KeyCode::Space => {
                if read_only {
                    handled = true;
                } else {
                    let (s, e) = sel.unwrap_or((caret, caret));
                    if let Some(inserted) =
                        Self::prepare_inserted_text(semantics, &value, s, e, " ")
                    {
                        next_caret = s + inserted.len();
                        next_anchor = next_caret;
                        next_edit = Some((s..e, inserted));
                    }
                    handled = true;
                }
            }
            KeyCode::Char(ch) => {
                let lower = ch.to_ascii_lowercase();
                if primary_shortcut {
                    let command = match lower {
                        'a' => Some(EditingCommand::SelectAll),
                        'c' => Some(EditingCommand::Copy),
                        'x' => Some(EditingCommand::Cut),
                        'v' => Some(EditingCommand::Paste(
                            ctx.clipboard
                                .and_then(|clipboard| clipboard.get_text())
                                .unwrap_or_default(),
                        )),
                        'z' if shift => Some(EditingCommand::Redo),
                        'z' => Some(EditingCommand::Undo),
                        'y' if !is_apple => Some(EditingCommand::Redo),
                        _ => None,
                    };
                    return command
                        .map_or(true, |command| self.handle_editing_command(ctx, &command));
                }

                if !handled
                    && is_apple
                    && Self::has_ctrl(modifiers)
                    && !Self::has_alt(modifiers)
                    && !Self::has_super(modifiers)
                {
                    match lower {
                        'a' => {
                            let (line_start, _) = Self::current_line_bounds(
                                ctx, focused_id, semantics, &value, caret,
                            );
                            next_caret = line_start;
                            next_anchor = if shift { anchor } else { line_start };
                            handled = true;
                        }
                        'e' => {
                            let (_, line_end) = Self::current_line_bounds(
                                ctx, focused_id, semantics, &value, caret,
                            );
                            next_caret = line_end;
                            next_anchor = if shift { anchor } else { line_end };
                            handled = true;
                        }
                        'f' => {
                            let next = Self::next_grapheme_boundary(&value, caret);
                            next_caret = next;
                            next_anchor = if shift { anchor } else { next };
                            handled = true;
                        }
                        'b' => {
                            let prev = Self::prev_grapheme_boundary(&value, caret);
                            next_caret = prev;
                            next_anchor = if shift { anchor } else { prev };
                            handled = true;
                        }
                        'n' if semantics.multiline => {
                            self.handle_vertical_navigation(
                                ctx, focused_id, semantics, &value, caret, modifiers, false,
                            );
                            return true;
                        }
                        'p' if semantics.multiline => {
                            self.handle_vertical_navigation(
                                ctx, focused_id, semantics, &value, caret, modifiers, true,
                            );
                            return true;
                        }
                        'h' => {
                            handled = true;
                            if !read_only {
                                let (s, e) = sel.unwrap_or_else(|| {
                                    if caret == 0 {
                                        (0, 0)
                                    } else {
                                        (Self::prev_grapheme_boundary(&value, caret), caret)
                                    }
                                });
                                next_edit = Some((s..e, String::new()));
                                next_caret = s;
                                next_anchor = s;
                            }
                        }
                        'd' => {
                            handled = true;
                            if !read_only {
                                let (s, e) = sel.unwrap_or_else(|| {
                                    let next = Self::next_grapheme_boundary(&value, caret);
                                    (caret, next)
                                });
                                next_edit = Some((s..e, String::new()));
                                next_caret = s;
                                next_anchor = s;
                            }
                        }
                        _ => {}
                    }
                }

                if !handled {
                    if read_only {
                        handled = true;
                    } else {
                        let (s, e) = sel.unwrap_or((caret, caret));
                        if let Some(inserted) =
                            Self::prepare_inserted_text(semantics, &value, s, e, &ch.to_string())
                        {
                            next_caret = s + inserted.len();
                            next_anchor = next_caret;
                            next_edit = Some((s..e, inserted));
                        }
                        handled = true;
                    }
                }
            }
            KeyCode::Backspace => {
                handled = true;
                if !read_only {
                    let (s, e) = if let Some((s, e)) = sel {
                        (s, e)
                    } else if is_apple && Self::has_super(modifiers) {
                        let (line_start, _) =
                            Self::current_line_bounds(ctx, focused_id, semantics, &value, caret);
                        (line_start, caret)
                    } else if word_modifier {
                        (Self::prev_word_boundary(&value, caret), caret)
                    } else if caret == 0 {
                        (0, 0)
                    } else {
                        (Self::prev_grapheme_boundary(&value, caret), caret)
                    };
                    next_edit = Some((s..e, String::new()));
                    next_caret = s;
                    next_anchor = s;
                }
            }
            KeyCode::Delete => {
                handled = true;
                if !read_only {
                    let (s, e) = if let Some((s, e)) = sel {
                        (s, e)
                    } else if is_apple && Self::has_super(modifiers) {
                        let (_, line_end) =
                            Self::current_line_bounds(ctx, focused_id, semantics, &value, caret);
                        (caret, line_end)
                    } else if word_modifier {
                        (caret, Self::next_word_boundary(&value, caret))
                    } else {
                        let next = Self::next_grapheme_boundary(&value, caret);
                        (caret, next)
                    };
                    next_edit = Some((s..e, String::new()));
                    next_caret = s;
                    next_anchor = s;
                }
            }
            KeyCode::Left => {
                let prev = if let Some((s, _)) = sel {
                    if !shift && !word_modifier && !(is_apple && Self::has_super(modifiers)) {
                        s
                    } else if is_apple && Self::has_super(modifiers) {
                        Self::current_line_bounds(ctx, focused_id, semantics, &value, caret).0
                    } else if word_modifier {
                        Self::prev_word_boundary(&value, caret)
                    } else {
                        Self::prev_grapheme_boundary(&value, caret)
                    }
                } else if is_apple && Self::has_super(modifiers) {
                    Self::current_line_bounds(ctx, focused_id, semantics, &value, caret).0
                } else if word_modifier {
                    Self::prev_word_boundary(&value, caret)
                } else {
                    Self::prev_grapheme_boundary(&value, caret)
                };
                next_caret = prev;
                next_anchor = if shift { anchor } else { prev };
                handled = true;
            }
            KeyCode::Right => {
                let next = if let Some((_, e)) = sel {
                    if !shift && !word_modifier && !(is_apple && Self::has_super(modifiers)) {
                        e
                    } else if is_apple && Self::has_super(modifiers) {
                        Self::current_line_bounds(ctx, focused_id, semantics, &value, caret).1
                    } else if word_modifier {
                        Self::next_word_boundary(&value, caret)
                    } else {
                        Self::next_grapheme_boundary(&value, caret)
                    }
                } else if is_apple && Self::has_super(modifiers) {
                    Self::current_line_bounds(ctx, focused_id, semantics, &value, caret).1
                } else if word_modifier {
                    Self::next_word_boundary(&value, caret)
                } else {
                    Self::next_grapheme_boundary(&value, caret)
                };
                next_caret = next;
                next_anchor = if shift { anchor } else { next };
                handled = true;
            }
            KeyCode::Home => {
                next_caret = if semantics.multiline && !(Self::has_ctrl(modifiers) && !is_apple) {
                    Self::current_line_bounds(ctx, focused_id, semantics, &value, caret).0
                } else {
                    0
                };
                next_anchor = if shift { anchor } else { next_caret };
                handled = true;
            }
            KeyCode::End => {
                next_caret = if semantics.multiline && !(Self::has_ctrl(modifiers) && !is_apple) {
                    Self::current_line_bounds(ctx, focused_id, semantics, &value, caret).1
                } else {
                    value.len()
                };
                next_anchor = if shift { anchor } else { next_caret };
                handled = true;
            }
            KeyCode::Enter => {
                if semantics.multiline {
                    handled = true;
                    if !read_only {
                        let insert_str = if semantics.auto_indent {
                            let line_start = value[..caret].rfind('\n').map(|p| p + 1).unwrap_or(0);
                            let leading: String = value[line_start..]
                                .chars()
                                .take_while(|c| *c == ' ' || *c == '\t')
                                .collect();
                            format!("\n{}", leading)
                        } else {
                            "\n".to_string()
                        };
                        let (s, e) = sel.unwrap_or((caret, caret));
                        if let Some(inserted) =
                            Self::prepare_inserted_text(semantics, &value, s, e, &insert_str)
                        {
                            next_caret = s + inserted.len();
                            next_anchor = next_caret;
                            next_edit = Some((s..e, inserted));
                        }
                    }
                } else if Self::dispatch_submit(ctx, semantics, focused_id, &value) {
                    return true;
                }
            }
            KeyCode::Up => {
                if is_apple && Self::has_super(modifiers) {
                    next_caret = 0;
                    next_anchor = if shift { anchor } else { 0 };
                    handled = true;
                } else if semantics.multiline {
                    self.handle_vertical_navigation(
                        ctx, focused_id, semantics, &value, caret, modifiers, true,
                    );
                    return true;
                }
            }
            KeyCode::Down => {
                if is_apple && Self::has_super(modifiers) {
                    next_caret = value.len();
                    next_anchor = if shift { anchor } else { value.len() };
                    handled = true;
                } else if semantics.multiline {
                    self.handle_vertical_navigation(
                        ctx, focused_id, semantics, &value, caret, modifiers, false,
                    );
                    return true;
                }
            }
            KeyCode::PageUp => {
                if semantics.multiline {
                    self.handle_page_navigation(
                        ctx, focused_id, semantics, &value, caret, modifiers, true,
                    );
                    return true;
                }
            }
            KeyCode::PageDown => {
                if semantics.multiline {
                    self.handle_page_navigation(
                        ctx, focused_id, semantics, &value, caret, modifiers, false,
                    );
                    return true;
                }
            }
            KeyCode::Tab => {
                if semantics.capture_tab {
                    handled = true;
                    if !read_only {
                        let tab_str = "    ";
                        let (s, e) = sel.unwrap_or((caret, caret));
                        if let Some(inserted) =
                            Self::prepare_inserted_text(semantics, &value, s, e, tab_str)
                        {
                            next_caret = s + inserted.len();
                            next_anchor = next_caret;
                            next_edit = Some((s..e, inserted));
                        }
                    }
                }
            }
            _ => {}
        }

        if let Some((range, replacement)) = next_edit {
            if let Some(result) = Self::apply_text_edit_transaction(
                ctx,
                semantics,
                focused_id,
                range,
                replacement,
                crate::TextEditSource::Keyboard,
            ) {
                next_caret = result.new_value.selection.extent.utf8_offset();
                next_anchor = result.new_value.selection.base.utf8_offset();
                self.dispatch_edit_result(ctx, semantics, focused_id, result);
                Self::dispatch_cursor_change(ctx, semantics, focused_id, next_caret, next_anchor);
            }
            Self::sync_text_input_affordances(
                ctx,
                focused_id,
                semantics,
                value.as_str(),
                false,
                None,
            );
        } else if handled {
            // Cursor movement only
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            st.caret = next_caret;
            st.anchor = next_anchor;
            st.clear_preedit();
            Self::auto_scroll_textinput(ctx, focused_id);
            Self::dispatch_cursor_change(ctx, semantics, focused_id, next_caret, next_anchor);
            Self::sync_text_input_affordances(
                ctx,
                focused_id,
                semantics,
                value.as_str(),
                false,
                None,
            );
        }

        handled
    }

    fn runtime_config(
        ctx: &ControllerContext,
        focused_id: WidgetId,
    ) -> Option<crate::ui::widgets::text_input::TextInputRuntimeConfig> {
        ctx.ir
            .custom_render_objects
            .get(&focused_id)
            .and_then(downcast_text_input_runtime_config)
            .cloned()
    }

    fn drag_start_behavior(ctx: &ControllerContext, focused_id: WidgetId) -> DragStartBehavior {
        Self::runtime_config(ctx, focused_id)
            .map(|cfg| cfg.drag_start_behavior)
            .unwrap_or_default()
    }

    fn sync_runtime_state(ctx: &mut ControllerContext, focused_id: WidgetId, semantic_value: &str) {
        let runtime = Self::runtime_config(ctx, focused_id);
        let masked =
            Self::text_input_semantics(ctx, focused_id).is_some_and(|semantics| semantics.masked);
        ctx.text_edit.sync_from_runtime(
            focused_id,
            semantic_value,
            runtime
                .as_ref()
                .and_then(|cfg| cfg.restoration_id.as_deref()),
            runtime
                .as_ref()
                .and_then(|cfg| cfg.undo_controller.as_ref().map(|undo| undo.capacity)),
            masked,
        );
    }

    fn persist_runtime_state(ctx: &mut ControllerContext, focused_id: WidgetId) {
        let runtime = Self::runtime_config(ctx, focused_id);
        let masked =
            Self::text_input_semantics(ctx, focused_id).is_some_and(|semantics| semantics.masked);
        ctx.text_edit.persist_restoration(
            focused_id,
            runtime
                .as_ref()
                .and_then(|cfg| cfg.restoration_id.as_deref()),
            masked,
        );
    }

    fn has_shift(modifiers: u8) -> bool {
        (modifiers & MOD_SHIFT) != 0
    }

    fn has_alt(modifiers: u8) -> bool {
        (modifiers & MOD_ALT) != 0
    }

    fn has_ctrl(modifiers: u8) -> bool {
        (modifiers & MOD_CTRL) != 0
    }

    fn has_super(modifiers: u8) -> bool {
        (modifiers & MOD_SUPER) != 0
    }

    fn node_or_ancestor_matches(
        ir: &fission_ir::CoreIR,
        node_id: WidgetId,
        expected: WidgetId,
    ) -> bool {
        let mut current = Some(node_id);
        while let Some(id) = current {
            if id == expected {
                return true;
            }
            current = ir.nodes.get(&id).and_then(|node| node.parent);
        }
        false
    }

    fn toolbar_action_hit(
        ir: &fission_ir::CoreIR,
        focused_id: WidgetId,
        hit_node_id: WidgetId,
    ) -> Option<TextContextMenuAction> {
        for action in [
            TextContextMenuAction::Copy,
            TextContextMenuAction::Cut,
            TextContextMenuAction::Paste,
            TextContextMenuAction::SelectAll,
        ] {
            if Self::node_or_ancestor_matches(
                ir,
                hit_node_id,
                text_input_toolbar_button_id(focused_id, action),
            ) {
                return Some(action);
            }
        }
        None
    }

    fn selection_handle_hit(
        ir: &fission_ir::CoreIR,
        focused_id: WidgetId,
        hit_node_id: WidgetId,
    ) -> Option<TextSelectionHandleKind> {
        for kind in [
            TextSelectionHandleKind::Caret,
            TextSelectionHandleKind::Start,
            TextSelectionHandleKind::End,
        ] {
            if Self::node_or_ancestor_matches(
                ir,
                hit_node_id,
                text_input_selection_handle_id(focused_id, kind),
            ) {
                return Some(kind);
            }
        }
        None
    }

    fn execute_toolbar_action(
        &mut self,
        ctx: &mut ControllerContext,
        action: TextContextMenuAction,
    ) -> bool {
        let command = match action {
            TextContextMenuAction::Copy => EditingCommand::Copy,
            TextContextMenuAction::Cut => EditingCommand::Cut,
            TextContextMenuAction::Paste => EditingCommand::Paste(
                ctx.clipboard
                    .and_then(|clipboard| clipboard.get_text())
                    .unwrap_or_default(),
            ),
            TextContextMenuAction::SelectAll => EditingCommand::SelectAll,
        };
        self.handle_editing_command(ctx, &command)
    }

    fn prepare_inserted_text(
        semantics: &Semantics,
        _current_value: &str,
        _replace_start: usize,
        _replace_end: usize,
        raw_text: &str,
    ) -> Option<String> {
        // Keyboard type and capitalization are platform intent, not validators.
        // Only explicit structural constraints may alter inserted text here.
        let mut inserted = raw_text.to_string();
        if !semantics.multiline {
            inserted = inserted.replace(['\r', '\n'], "");
        }

        if let Some(mask) = &semantics.input_mask {
            inserted = inserted
                .chars()
                .filter(|ch| mask.is_valid_char(*ch))
                .collect();
        }

        if inserted.is_empty() {
            None
        } else {
            Some(inserted)
        }
    }

    fn apply_text_edit_transaction(
        ctx: &mut ControllerContext,
        semantics: &Semantics,
        node_id: WidgetId,
        range: std::ops::Range<usize>,
        replacement: String,
        source: crate::TextEditSource,
    ) -> Option<crate::TextEditResult> {
        let runtime = Self::runtime_config(ctx, node_id);
        let old_value = ctx.text_edit.get_mut_or_default(node_id).editing_value();
        let range = crate::TextRange::new(&old_value.text, range.start, range.end).ok()?;
        let mut formatters = semantics.input_formatters.clone();
        if !semantics.multiline && !formatters.contains(&InputFormatter::SingleLine) {
            formatters.push(InputFormatter::SingleLine);
        }
        let pipeline = crate::TextEditPipeline {
            formatters,
            custom_formatters: runtime
                .map(|config| config.custom_input_formatters)
                .unwrap_or_default(),
            max_length: semantics.max_length,
            max_length_enforcement: semantics.max_length_enforcement,
        };
        let mut result = pipeline
            .apply(
                &old_value,
                crate::TextEditCommand::Replace {
                    range,
                    text: replacement,
                    source,
                },
            )
            .ok()?;
        if source == crate::TextEditSource::Ime {
            result.phase = crate::TextEditPhase::CompositionCommitted;
        }
        if result.new_value == result.old_value {
            return None;
        }
        ctx.text_edit
            .get_mut_or_default(node_id)
            .apply_editing_value(result.new_value.clone());
        Some(result)
    }

    fn dispatch_edit_result(
        &self,
        ctx: &mut ControllerContext,
        semantics: &Semantics,
        node_id: WidgetId,
        result: crate::TextEditResult,
    ) {
        Self::persist_runtime_state(ctx, node_id);
        if let Some((envelope, input)) =
            crate::input::prepare_scoped_text_input_edit(ctx.ir, semantics, node_id, result)
        {
            ctx.dispatched_actions.push((node_id, envelope, input));
            Self::auto_scroll_textinput(ctx, node_id);
        }
    }

    fn handle_ime(&mut self, ctx: &mut ControllerContext, ime: &crate::event::ImeEvent) -> bool {
        match ime {
            crate::event::ImeEvent::Commit { text } => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.role == fission_ir::semantics::Role::TextInput {
                                if semantics.disabled || semantics.read_only {
                                    return true;
                                }
                                Self::sync_runtime_state(
                                    ctx,
                                    focused_id,
                                    semantics.value.as_deref().unwrap_or(""),
                                );
                                let (value, start, end) = {
                                    let st = ctx.text_edit.get_mut_or_default(focused_id);
                                    if let Some(preedit) = &st.preedit {
                                        let (start, end) = preedit.range;
                                        let committed = st.committed_text();
                                        // Legacy IME preedit is a display projection over the
                                        // committed buffer. Remove that projection before the
                                        // canonical transaction replaces its original range;
                                        // otherwise the replacement is compared with the already
                                        // projected display value and is incorrectly treated as a
                                        // no-op.
                                        st.clear_preedit();
                                        (committed, start, end)
                                    } else {
                                        let value = st.editing_value();
                                        let range = value.selection_range();
                                        (
                                            value.text,
                                            range.start.utf8_offset(),
                                            range.end.utf8_offset(),
                                        )
                                    }
                                };

                                if let Some(filtered_text) =
                                    Self::prepare_inserted_text(semantics, &value, start, end, text)
                                {
                                    if let Some(result) = Self::apply_text_edit_transaction(
                                        ctx,
                                        semantics,
                                        focused_id,
                                        start..end,
                                        filtered_text,
                                        crate::TextEditSource::Ime,
                                    ) {
                                        let caret = result.new_value.selection.extent.utf8_offset();
                                        let anchor = result.new_value.selection.base.utf8_offset();
                                        self.dispatch_edit_result(
                                            ctx, semantics, focused_id, result,
                                        );
                                        Self::dispatch_cursor_change(
                                            ctx, semantics, focused_id, caret, anchor,
                                        );
                                    }
                                } else {
                                    ctx.text_edit.get_mut_or_default(focused_id).clear_preedit();
                                }

                                return true;
                            }
                        }
                    }
                }
            }
            crate::event::ImeEvent::Preedit { text, cursor } => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.disabled || semantics.read_only {
                                return true;
                            }
                            Self::sync_runtime_state(
                                ctx,
                                focused_id,
                                semantics.value.as_deref().unwrap_or(""),
                            );
                        }
                    }
                    let st = ctx.text_edit.get_mut_or_default(focused_id);
                    st.set_preedit(text.clone(), *cursor);
                    Self::auto_scroll_textinput(ctx, focused_id);
                    return true;
                }
            }
            crate::event::ImeEvent::Cancel => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.disabled || semantics.read_only {
                                return true;
                            }
                            Self::sync_runtime_state(
                                ctx,
                                focused_id,
                                semantics.value.as_deref().unwrap_or(""),
                            );
                        }
                    }
                    let st = ctx.text_edit.get_mut_or_default(focused_id);
                    st.clear_preedit();
                    Self::auto_scroll_textinput(ctx, focused_id);
                    return true;
                }
            }
        }
        false
    }

    fn dispatch_change(
        &self,
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        new_text: String,
    ) {
        Self::persist_runtime_state(ctx, node_id);
        let (new_caret, new_anchor) = ctx
            .text_edit
            .get(node_id)
            .map(|state| (state.caret, state.anchor))
            .unwrap_or((new_text.len(), new_text.len()));
        if let Some((envelope, input)) = crate::input::prepare_scoped_text_input_change(
            ctx.ir, semantics, node_id, new_text, new_caret, new_anchor,
        ) {
            ctx.dispatched_actions.push((node_id, envelope, input));

            // State update moved to handle_key to avoid double borrow

            Self::auto_scroll_textinput(ctx, node_id);
        }
    }

    fn dispatch_cursor_change(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        new_caret: usize,
        new_anchor: usize,
    ) {
        Self::dispatch_cursor_change_from(
            ctx,
            semantics,
            node_id,
            new_caret,
            new_anchor,
            crate::TextEditSource::Programmatic,
        );
    }

    fn dispatch_cursor_change_from(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        new_caret: usize,
        new_anchor: usize,
        source: crate::TextEditSource,
    ) {
        // Deduplicate: skip dispatch if cursor position hasn't actually changed
        // since our last dispatch. This prevents unnecessary model updates that
        // would trigger extra rebuild cycles.
        if let Some(st) = ctx.text_edit.states.get(&node_id) {
            if st.last_dispatched_cursor == Some((new_caret, new_anchor)) {
                return;
            }
        }

        Self::persist_runtime_state(ctx, node_id);

        if let Some(action_entry) = semantics
            .actions
            .entries
            .iter()
            .find(|e| e.trigger == fission_ir::semantics::ActionTrigger::CursorChange)
        {
            // Record the dispatched position before dispatching
            if let Some(st) = ctx.text_edit.states.get_mut(&node_id) {
                st.last_dispatched_cursor = Some((new_caret, new_anchor));
            }

            let payload = action_entry.payload_data.clone().unwrap_or_default();
            let envelope = ActionEnvelope {
                id: ActionId::from_u128(action_entry.action_id),
                payload,
            };
            let mut value = ctx.text_edit.get_mut_or_default(node_id).editing_value();
            if let Ok(selection) = crate::TextSelection::new(
                &value.text,
                new_anchor,
                new_caret,
                crate::TextAffinity::Downstream,
            ) {
                value.selection = selection;
            }
            let input = crate::input::scoped_action_input(
                ctx.ir,
                node_id,
                crate::ActionInput::TextSelectionChanged(crate::action::UpdateTextSelection {
                    node_id,
                    value,
                    source,
                }),
            );
            ctx.dispatched_actions.push((node_id, envelope, input));
        }
    }

    fn dispatch_submit(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        current_value: &str,
    ) -> bool {
        let mut dispatched = false;
        for trigger in [
            fission_ir::semantics::ActionTrigger::Validation,
            fission_ir::semantics::ActionTrigger::EditingComplete,
            fission_ir::semantics::ActionTrigger::Submit,
        ] {
            dispatched |= Self::dispatch_action_for_trigger(
                ctx,
                semantics,
                node_id,
                trigger,
                Some(serde_json::to_vec(&current_value.to_string()).unwrap()),
            );
        }
        dispatched
    }

    fn dispatch_action_for_trigger(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: WidgetId,
        trigger: fission_ir::semantics::ActionTrigger,
        fallback_payload: Option<Vec<u8>>,
    ) -> bool {
        let Some(action_entry) = semantics
            .actions
            .entries
            .iter()
            .find(|e| e.trigger == trigger)
        else {
            return false;
        };
        let payload = action_entry
            .payload_data
            .clone()
            .or(fallback_payload)
            .unwrap_or_else(|| serde_json::to_vec(&()).unwrap());
        let envelope = ActionEnvelope {
            id: ActionId::from_u128(action_entry.action_id),
            payload,
        };
        let dynamic_input = match trigger {
            fission_ir::semantics::ActionTrigger::Submit
            | fission_ir::semantics::ActionTrigger::EditingComplete
            | fission_ir::semantics::ActionTrigger::Validation
            | fission_ir::semantics::ActionTrigger::TapOutside
            | fission_ir::semantics::ActionTrigger::Focus
            | fission_ir::semantics::ActionTrigger::Blur => {
                let value = ctx.text_edit.get_mut_or_default(node_id).editing_value();
                let (source, phase) = match trigger {
                    fission_ir::semantics::ActionTrigger::Submit => (
                        crate::TextEditSource::Keyboard,
                        crate::TextEditPhase::Submitted,
                    ),
                    fission_ir::semantics::ActionTrigger::EditingComplete => (
                        crate::TextEditSource::Keyboard,
                        crate::TextEditPhase::EditingCompleted,
                    ),
                    fission_ir::semantics::ActionTrigger::Validation => (
                        crate::TextEditSource::Programmatic,
                        crate::TextEditPhase::Validated,
                    ),
                    fission_ir::semantics::ActionTrigger::TapOutside => (
                        crate::TextEditSource::Pointer,
                        crate::TextEditPhase::TapOutside,
                    ),
                    fission_ir::semantics::ActionTrigger::Focus => (
                        crate::TextEditSource::Pointer,
                        crate::TextEditPhase::Focused,
                    ),
                    fission_ir::semantics::ActionTrigger::Blur => (
                        crate::TextEditSource::Pointer,
                        crate::TextEditPhase::Blurred,
                    ),
                    _ => unreachable!(),
                };
                let mut input = crate::UpdateTextInput::from_values(
                    node_id,
                    value.clone(),
                    value,
                    source,
                    phase,
                );
                if matches!(
                    trigger,
                    fission_ir::semantics::ActionTrigger::Submit
                        | fission_ir::semantics::ActionTrigger::EditingComplete
                ) {
                    input.editing_action = Some(semantics.text_input_action);
                }
                if trigger == fission_ir::semantics::ActionTrigger::Validation {
                    input.validation_state = Some(semantics.validation_state);
                    input.validation_message = semantics.validation_message.clone();
                }
                crate::ActionInput::TextChanged(input)
            }
            _ => crate::ActionInput::None,
        };
        let input = crate::input::scoped_action_input(ctx.ir, node_id, dynamic_input);
        ctx.dispatched_actions.push((node_id, envelope, input));
        true
    }
}

mod geometry;
pub use geometry::caret_from_point_in_text;
