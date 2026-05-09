use super::{ControllerContext, InputController};
use crate::env::TextSelectionHandleKind;
use crate::event::{
    InputEvent, KeyCode, KeyEvent, PointerEvent, MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_SUPER,
};
use crate::ui::widgets::text_input::{
    downcast_text_input_runtime_config, text_input_selection_handle_id,
    text_input_toolbar_button_id, DragStartBehavior, TextContextMenuAction,
};
use crate::ActionEnvelope;
use crate::ActionId;
use fission_ir::FlexDirection;
use fission_ir::{
    op::{self, decode_text_paragraph_style, LayoutOp, Op, TextAlign, TextParagraphStyle},
    semantics::{InputFormatter, MaxLengthEnforcement, TextCapitalization, TextInputType},
    NodeId, Semantics,
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
                                if let Some((scroll_id, _text_op_node_id, _scroll_direction)) =
                                    scroll_result
                                {
                                    if let Some(scroll_geom) =
                                        ctx.layout.get_node_geometry(scroll_id)
                                    {
                                        let value = sem.value.as_deref().unwrap_or("");
                                        let display_value =
                                            Self::display_value_for_metrics(ctx, focused_id, value);
                                        let metric_text = if sem.masked {
                                            Self::mask_text_for_metrics(&display_value)
                                        } else {
                                            display_value.clone()
                                        };
                                        let offset = ctx.scroll.get_offset(scroll_id);

                                        // Accumulate ancestor scroll offsets to convert
                                        // screen coordinates to local content coordinates.
                                        let mut ancestor_scroll_y = 0.0f32;
                                        let mut ancestor_scroll_x = 0.0f32;
                                        {
                                            let mut walk =
                                                ctx.ir.nodes.get(&scroll_id).and_then(|n| n.parent);
                                            while let Some(pid) = walk {
                                                if let Some(pnode) = ctx.ir.nodes.get(&pid) {
                                                    if let Op::Layout(LayoutOp::Scroll {
                                                        direction,
                                                        ..
                                                    }) = &pnode.op
                                                    {
                                                        let poff = ctx.scroll.get_offset(pid);
                                                        match direction {
                                                            FlexDirection::Row => {
                                                                ancestor_scroll_x += poff
                                                            }
                                                            FlexDirection::Column => {
                                                                ancestor_scroll_y += poff
                                                            }
                                                        }
                                                    }
                                                    walk = pnode.parent;
                                                } else {
                                                    break;
                                                }
                                            }
                                        }

                                        let caret = if let Some(measurer) = ctx.measurer {
                                            let local_x = point.x - scroll_geom.rect.origin.x
                                                + offset
                                                + ancestor_scroll_x;
                                            let local_y = point.y - scroll_geom.rect.origin.y
                                                + ancestor_scroll_y;

                                            let masked_caret = Self::hit_test_text(
                                                measurer,
                                                ctx.ir,
                                                focused_id,
                                                sem.masked,
                                                &metric_text,
                                                scroll_geom,
                                                local_x,
                                                local_y,
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
                                                offset,
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
                                    if let Some((scroll_id, _text_op_node_id, _scroll_direction)) =
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
                                            let offset = ctx.scroll.get_offset(scroll_id);
                                            let new_caret = if let Some(measurer) = ctx.measurer {
                                                let local_x =
                                                    point.x - scroll_geom.rect.origin.x + offset;
                                                let local_y = point.y - scroll_geom.rect.origin.y;
                                                let masked_caret = Self::hit_test_text(
                                                    measurer,
                                                    ctx.ir,
                                                    focused_id,
                                                    sem.masked,
                                                    &metric_text,
                                                    scroll_geom,
                                                    local_x,
                                                    local_y,
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

                                if !ctx.interaction.pressed.is_empty() {
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
                                            _scroll_direction,
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
                                                let offset = ctx.scroll.get_offset(scroll_id);
                                                let new_caret = if let Some(measurer) = ctx.measurer
                                                {
                                                    // Accumulate ancestor scroll offsets for
                                                    // pointer-move the same way as pointer-down.
                                                    let mut anc_scroll_y = 0.0f32;
                                                    let mut anc_scroll_x = 0.0f32;
                                                    {
                                                        let mut walk = ctx
                                                            .ir
                                                            .nodes
                                                            .get(&scroll_id)
                                                            .and_then(|n| n.parent);
                                                        while let Some(pid) = walk {
                                                            if let Some(pnode) =
                                                                ctx.ir.nodes.get(&pid)
                                                            {
                                                                if let Op::Layout(
                                                                    LayoutOp::Scroll {
                                                                        direction,
                                                                        ..
                                                                    },
                                                                ) = &pnode.op
                                                                {
                                                                    let poff =
                                                                        ctx.scroll.get_offset(pid);
                                                                    match direction {
                                                                        FlexDirection::Row => {
                                                                            anc_scroll_x += poff
                                                                        }
                                                                        FlexDirection::Column => {
                                                                            anc_scroll_y += poff
                                                                        }
                                                                    }
                                                                }
                                                                walk = pnode.parent;
                                                            } else {
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    let local_x = point.x
                                                        - scroll_geom.rect.origin.x
                                                        + offset
                                                        + anc_scroll_x;
                                                    let local_y = point.y
                                                        - scroll_geom.rect.origin.y
                                                        + anc_scroll_y;

                                                    let masked_caret = Self::hit_test_text(
                                                        measurer,
                                                        ctx.ir,
                                                        focused_id,
                                                        sem.masked,
                                                        &metric_text,
                                                        scroll_geom,
                                                        local_x,
                                                        local_y,
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
                                                        offset,
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

        // Undo/Redo logic result
        let mut undo_redo_result: Option<(String, usize, usize)> = None;
        let read_only = semantics.read_only;
        let disabled = semantics.disabled;
        let is_apple = Self::is_apple_platform();
        let shift = Self::has_shift(modifiers);
        let primary_shortcut = Self::has_primary_shortcut(modifiers);
        let word_modifier = Self::has_word_modifier(modifiers);

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
                    let (s, e) = sel.unwrap_or((caret, caret));
                    match lower {
                        'a' => {
                            next_caret = value.len();
                            next_anchor = 0;
                            handled = true;
                        }
                        'c' => {
                            if s != e {
                                let txt = value[s..e].to_string();
                                if let Some(cb) = ctx.clipboard {
                                    cb.set_text(&txt);
                                }
                            }
                            handled = true;
                        }
                        'x' => {
                            if s != e {
                                let txt = value[s..e].to_string();
                                if let Some(cb) = ctx.clipboard {
                                    cb.set_text(&txt);
                                }
                                if !read_only {
                                    next_edit = Some((s..e, String::new()));
                                    next_caret = s;
                                    next_anchor = s;
                                }
                            }
                            handled = true;
                        }
                        'v' => {
                            handled = true;
                            if !read_only {
                                let text_to_paste = if let Some(cb) = ctx.clipboard {
                                    cb.get_text().unwrap_or_default()
                                } else {
                                    String::new()
                                };
                                if !text_to_paste.is_empty() {
                                    if let Some(inserted) = Self::prepare_inserted_text(
                                        semantics,
                                        &value,
                                        s,
                                        e,
                                        &text_to_paste,
                                    ) {
                                        next_caret = s + inserted.len();
                                        next_anchor = next_caret;
                                        next_edit = Some((s..e, inserted));
                                    }
                                }
                            }
                        }
                        'z' => {
                            let st = ctx.text_edit.get_mut_or_default(focused_id);
                            if shift {
                                if let Some((v, c, a)) = st.redo() {
                                    undo_redo_result = Some((v, c, a));
                                }
                            } else if let Some((v, c, a)) = st.undo() {
                                undo_redo_result = Some((v, c, a));
                            }
                            handled = true;
                        }
                        'y' if !is_apple => {
                            let st = ctx.text_edit.get_mut_or_default(focused_id);
                            if let Some((v, c, a)) = st.redo() {
                                undo_redo_result = Some((v, c, a));
                            }
                            handled = true;
                        }
                        _ => {}
                    }
                    if handled {
                        // Skip plain text insertion when a primary shortcut matched.
                    }
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

        if let Some((v, c, a)) = undo_redo_result {
            // Apply undo/redo result
            self.dispatch_change(ctx, semantics, focused_id, v);
            Self::dispatch_cursor_change(ctx, semantics, focused_id, c, a);
            Self::sync_text_input_affordances(
                ctx,
                focused_id,
                semantics,
                value.as_str(),
                false,
                None,
            );
            return true;
        }

        if let Some((range, replacement)) = next_edit {
            // Apply text change
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            let txt = st.apply_edit(range, &replacement, next_caret, next_anchor);
            self.dispatch_change(ctx, semantics, focused_id, txt);
            Self::dispatch_cursor_change(ctx, semantics, focused_id, next_caret, next_anchor);
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

    fn is_apple_platform() -> bool {
        cfg!(target_os = "macos") || cfg!(target_os = "ios")
    }

    fn runtime_config(
        ctx: &ControllerContext,
        focused_id: NodeId,
    ) -> Option<crate::ui::widgets::text_input::TextInputRuntimeConfig> {
        ctx.ir
            .custom_render_objects
            .get(&focused_id)
            .and_then(downcast_text_input_runtime_config)
            .cloned()
    }

    fn drag_start_behavior(ctx: &ControllerContext, focused_id: NodeId) -> DragStartBehavior {
        Self::runtime_config(ctx, focused_id)
            .map(|cfg| cfg.drag_start_behavior)
            .unwrap_or_default()
    }

    fn sync_runtime_state(ctx: &mut ControllerContext, focused_id: NodeId, semantic_value: &str) {
        let runtime = Self::runtime_config(ctx, focused_id);
        ctx.text_edit.sync_from_runtime(
            focused_id,
            semantic_value,
            runtime
                .as_ref()
                .and_then(|cfg| cfg.restoration_id.as_deref()),
            runtime
                .as_ref()
                .and_then(|cfg| cfg.undo_controller.as_ref().map(|undo| undo.capacity)),
        );
    }

    fn persist_runtime_state(ctx: &mut ControllerContext, focused_id: NodeId) {
        let runtime = Self::runtime_config(ctx, focused_id);
        ctx.text_edit.persist_restoration(
            focused_id,
            runtime
                .as_ref()
                .and_then(|cfg| cfg.restoration_id.as_deref()),
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

    fn has_primary_shortcut(modifiers: u8) -> bool {
        if Self::is_apple_platform() {
            Self::has_super(modifiers)
        } else {
            Self::has_ctrl(modifiers)
        }
    }

    fn has_word_modifier(modifiers: u8) -> bool {
        if Self::is_apple_platform() {
            Self::has_alt(modifiers)
        } else {
            Self::has_ctrl(modifiers)
        }
    }

    fn primary_shortcut_modifier() -> u8 {
        if Self::is_apple_platform() {
            MOD_SUPER
        } else {
            MOD_CTRL
        }
    }

    fn node_or_ancestor_matches(
        ir: &fission_ir::CoreIR,
        node_id: NodeId,
        expected: NodeId,
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
        focused_id: NodeId,
        hit_node_id: NodeId,
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
        focused_id: NodeId,
        hit_node_id: NodeId,
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
        match action {
            TextContextMenuAction::Copy => {
                self.handle_key(ctx, KeyCode::Char('c'), Self::primary_shortcut_modifier())
            }
            TextContextMenuAction::Cut => {
                self.handle_key(ctx, KeyCode::Char('x'), Self::primary_shortcut_modifier())
            }
            TextContextMenuAction::Paste => {
                self.handle_key(ctx, KeyCode::Char('v'), Self::primary_shortcut_modifier())
            }
            TextContextMenuAction::SelectAll => {
                self.handle_key(ctx, KeyCode::Char('a'), Self::primary_shortcut_modifier())
            }
        }
    }

    fn input_wrapper_geometry<'a>(
        ctx: &'a ControllerContext<'_>,
        focused_id: NodeId,
    ) -> Option<&'a fission_layout::LayoutNodeGeometry> {
        let wrapper_id = ctx.ir.nodes.get(&focused_id)?.children.first().copied()?;
        ctx.layout.get_node_geometry(wrapper_id)
    }

    fn line_metric_for_index<'a>(
        line_metrics: &'a [fission_layout::LineMetric],
        caret_index: usize,
    ) -> Option<(usize, &'a fission_layout::LineMetric)> {
        line_metrics
            .iter()
            .enumerate()
            .find(|(_, line)| caret_index >= line.start_index && caret_index <= line.end_index)
            .or_else(|| line_metrics.iter().enumerate().last())
    }

    fn local_text_point_for_index(
        measurer: &std::sync::Arc<dyn fission_layout::TextMeasurer>,
        ir: &fission_ir::CoreIR,
        focused_id: NodeId,
        wrapper_geom: &fission_layout::LayoutNodeGeometry,
        scroll_geom: &fission_layout::LayoutNodeGeometry,
        scroll_direction: FlexDirection,
        scroll_offset: f32,
        metric_text: &str,
        metric_index: usize,
    ) -> Option<fission_layout::LayoutPoint> {
        let font_size = Self::extract_font_size(ir, focused_id).unwrap_or(16.0);
        let paragraph = Self::extract_paragraph_style(ir, focused_id).unwrap_or_default();
        let render_width = if scroll_direction == FlexDirection::Column {
            Some(scroll_geom.rect.size.width)
        } else {
            None
        };
        let (mut caret_x, caret_y) =
            measurer.get_caret_position(metric_text, font_size, render_width, metric_index);
        let line_metrics = measurer.get_line_metrics(metric_text, font_size, render_width);
        let (line_index, line_metric) = Self::line_metric_for_index(&line_metrics, metric_index)?;
        let is_last_line = line_index + 1 == line_metrics.len();
        if let Some(width) = render_width {
            caret_x +=
                Self::paragraph_line_x_offset(paragraph, width, line_metric.width, is_last_line);
        }

        let visible_x = if scroll_direction == FlexDirection::Row {
            caret_x - scroll_offset
        } else {
            caret_x
        };
        let visible_y = if scroll_direction == FlexDirection::Column {
            caret_y - scroll_offset
        } else {
            caret_y
        };

        let local_x = (scroll_geom.rect.origin.x - wrapper_geom.rect.origin.x) + visible_x;
        let local_y = (scroll_geom.rect.origin.y - wrapper_geom.rect.origin.y)
            + visible_y
            + line_metric.height.max(1.0);

        Some(fission_layout::LayoutPoint::new(local_x, local_y))
    }

    fn clear_text_input_affordances(ctx: &mut ControllerContext, focused_id: NodeId) {
        if let Some(state) = ctx.text_edit.states.get_mut(&focused_id) {
            state.affordances = Default::default();
        }
    }

    fn sync_text_input_affordances(
        ctx: &mut ControllerContext,
        focused_id: NodeId,
        semantics: &Semantics,
        value: &str,
        toolbar_visible: bool,
        toolbar_anchor_override: Option<fission_layout::LayoutPoint>,
    ) {
        let Some(measurer) = ctx.measurer else {
            Self::clear_text_input_affordances(ctx, focused_id);
            return;
        };
        let Some(wrapper_geom) = Self::input_wrapper_geometry(ctx, focused_id).cloned() else {
            Self::clear_text_input_affordances(ctx, focused_id);
            return;
        };
        let Some((scroll_id, _text_node_id, scroll_direction)) =
            Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
        else {
            Self::clear_text_input_affordances(ctx, focused_id);
            return;
        };
        let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id).cloned() else {
            Self::clear_text_input_affordances(ctx, focused_id);
            return;
        };

        let display_value = Self::display_value_for_metrics(
            ctx,
            focused_id,
            semantics.value.as_deref().unwrap_or(value),
        );
        let metric_text = if semantics.masked {
            Self::mask_text_for_metrics(&display_value)
        } else {
            display_value.clone()
        };
        let (caret, anchor, active_handle) = {
            let state = ctx.text_edit.get_mut_or_default(focused_id);
            (state.caret, state.anchor, state.affordances.active_handle)
        };

        let map_metric_index = |index: usize| {
            if semantics.masked {
                Self::masked_byte_offset_from_source(&display_value, &metric_text, index)
            } else {
                index.min(metric_text.len())
            }
        };

        let scroll_offset = ctx.scroll.get_offset(scroll_id);
        let caret_point = Self::local_text_point_for_index(
            measurer,
            ctx.ir,
            focused_id,
            &wrapper_geom,
            &scroll_geom,
            scroll_direction,
            scroll_offset,
            &metric_text,
            map_metric_index(caret),
        );
        let anchor_point = Self::local_text_point_for_index(
            measurer,
            ctx.ir,
            focused_id,
            &wrapper_geom,
            &scroll_geom,
            scroll_direction,
            scroll_offset,
            &metric_text,
            map_metric_index(anchor),
        );

        let selection_range = if caret == anchor {
            None
        } else {
            Some((caret.min(anchor), caret.max(anchor)))
        };

        let toolbar_anchor = if let Some(override_point) = toolbar_anchor_override {
            Some(override_point)
        } else {
            match (caret_point, anchor_point, selection_range) {
                (Some(caret_point), Some(anchor_point), Some(_)) => {
                    Some(fission_layout::LayoutPoint::new(
                        (caret_point.x + anchor_point.x) * 0.5,
                        caret_point.y.min(anchor_point.y),
                    ))
                }
                (Some(point), _, None) => Some(point),
                _ => None,
            }
        };

        let state = ctx.text_edit.get_mut_or_default(focused_id);
        state.affordances.toolbar_visible = toolbar_visible;
        state.affordances.toolbar_anchor = toolbar_anchor;
        state.affordances.magnifier_visible = active_handle.is_some();
        state.affordances.magnifier_anchor = match active_handle {
            Some(TextSelectionHandleKind::Caret) => caret_point,
            Some(TextSelectionHandleKind::Start) => anchor_point,
            Some(TextSelectionHandleKind::End) => caret_point,
            None => None,
        };
        if selection_range.is_some() {
            let (start_point, end_point) = if caret <= anchor {
                (caret_point, anchor_point)
            } else {
                (anchor_point, caret_point)
            };
            state.affordances.caret_handle = None;
            state.affordances.selection_start_handle = start_point;
            state.affordances.selection_end_handle = end_point;
        } else {
            state.affordances.caret_handle = caret_point;
            state.affordances.selection_start_handle = None;
            state.affordances.selection_end_handle = None;
        }
    }

    fn trim_line_end(value: &str, end: usize) -> usize {
        let end = end.min(value.len());
        if end > 0 && value.as_bytes()[end - 1] == b'\n' {
            end - 1
        } else {
            end
        }
    }

    fn current_line_bounds(
        ctx: &ControllerContext,
        focused_id: NodeId,
        semantics: &Semantics,
        value: &str,
        caret: usize,
    ) -> (usize, usize) {
        let caret = caret.min(value.len());
        if semantics.multiline {
            if let Some(measurer) = ctx.measurer {
                if let Some((scroll_id, _text_op_node_id, _scroll_direction)) =
                    Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
                {
                    if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                        let font_size = Self::extract_font_size(ctx.ir, focused_id).unwrap_or(16.0);
                        let line_metrics = measurer.get_line_metrics(
                            value,
                            font_size,
                            Some(scroll_geom.rect.size.width),
                        );
                        if let Some(line) = line_metrics
                            .iter()
                            .find(|line| caret >= line.start_index && caret <= line.end_index)
                            .or_else(|| line_metrics.last())
                        {
                            let start = line.start_index.min(value.len());
                            let end = Self::trim_line_end(value, line.end_index);
                            return (start.min(end), end);
                        }
                    }
                }
            }

            let start = value[..caret].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let end = value[caret..]
                .find('\n')
                .map(|offset| caret + offset)
                .unwrap_or(value.len());
            (start.min(end), end)
        } else {
            (0, value.len())
        }
    }

    fn truncate_to_chars(text: &str, max_chars: usize) -> String {
        text.chars().take(max_chars).collect()
    }

    fn apply_text_capitalization(mode: TextCapitalization, prefix: &str, inserted: &str) -> String {
        match mode {
            TextCapitalization::None => inserted.to_string(),
            TextCapitalization::Characters => inserted.to_uppercase(),
            TextCapitalization::Words => {
                let starts_new_word = prefix
                    .chars()
                    .next_back()
                    .map(|ch| ch.is_whitespace() || ch.is_ascii_punctuation())
                    .unwrap_or(true);
                if starts_new_word {
                    let mut chars = inserted.chars();
                    if let Some(first) = chars.next() {
                        let mut out = first.to_uppercase().to_string();
                        out.push_str(chars.as_str());
                        out
                    } else {
                        String::new()
                    }
                } else {
                    inserted.to_string()
                }
            }
            TextCapitalization::Sentences => {
                let starts_sentence = prefix
                    .chars()
                    .rev()
                    .find(|ch| !ch.is_whitespace())
                    .map(|ch| matches!(ch, '.' | '!' | '?'))
                    .unwrap_or(true);
                if starts_sentence {
                    let mut chars = inserted.chars();
                    if let Some(first) = chars.next() {
                        let mut out = first.to_uppercase().to_string();
                        out.push_str(chars.as_str());
                        out
                    } else {
                        String::new()
                    }
                } else {
                    inserted.to_string()
                }
            }
        }
    }

    fn apply_input_type_filter(input_type: TextInputType, text: &str, multiline: bool) -> String {
        let mut filtered = String::new();
        for ch in text.chars() {
            let allowed = match input_type {
                TextInputType::Text | TextInputType::Name => multiline || ch != '\n',
                TextInputType::Multiline => true,
                TextInputType::Number => ch.is_ascii_digit() || matches!(ch, '.' | ',' | '-' | '+'),
                TextInputType::EmailAddress => !ch.is_whitespace(),
                TextInputType::Url => !ch.is_whitespace(),
                TextInputType::Phone => {
                    ch.is_ascii_digit() || matches!(ch, '+' | '-' | '(' | ')' | ' ')
                }
            };
            if allowed {
                filtered.push(ch);
            }
        }
        if !multiline {
            filtered = filtered.replace('\n', "");
        }
        filtered
    }

    fn apply_formatters(text: &str, formatters: &[InputFormatter], multiline: bool) -> String {
        let mut out = text.to_string();
        for formatter in formatters {
            match formatter {
                InputFormatter::DigitsOnly => {
                    out = out.chars().filter(|ch| ch.is_ascii_digit()).collect();
                }
                InputFormatter::AsciiOnly => {
                    out = out.chars().filter(|ch| ch.is_ascii()).collect();
                }
                InputFormatter::Lowercase => {
                    out = out.to_lowercase();
                }
                InputFormatter::Uppercase => {
                    out = out.to_uppercase();
                }
                InputFormatter::TrimWhitespace => {
                    out = out.trim().to_string();
                }
                InputFormatter::SingleLine => {
                    out = out.replace('\n', "");
                }
            }
        }
        if !multiline {
            out = out.replace('\n', "");
        }
        out
    }

    fn prepare_inserted_text(
        semantics: &Semantics,
        current_value: &str,
        replace_start: usize,
        replace_end: usize,
        raw_text: &str,
    ) -> Option<String> {
        let replace_start = replace_start.min(current_value.len());
        let replace_end = replace_end.min(current_value.len()).max(replace_start);

        let mut inserted =
            Self::apply_input_type_filter(semantics.text_input_type, raw_text, semantics.multiline);
        inserted = Self::apply_text_capitalization(
            semantics.text_capitalization,
            &current_value[..replace_start],
            &inserted,
        );
        inserted =
            Self::apply_formatters(&inserted, &semantics.input_formatters, semantics.multiline);

        if let Some(mask) = &semantics.input_mask {
            inserted = inserted
                .chars()
                .filter(|ch| mask.is_valid_char(*ch))
                .collect();
        }

        if semantics.max_length_enforcement == MaxLengthEnforcement::Enforced {
            if let Some(max_length) = semantics.max_length {
                let current_chars = current_value.chars().count();
                let replaced_chars = current_value[replace_start..replace_end].chars().count();
                let available =
                    max_length.saturating_sub(current_chars.saturating_sub(replaced_chars));
                inserted = Self::truncate_to_chars(&inserted, available);
            }
        }

        if inserted.is_empty() {
            None
        } else {
            Some(inserted)
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
                                let (value, _caret, _anchor) = Self::resolve_editing_value(
                                    ctx,
                                    focused_id,
                                    semantics.value.as_deref().unwrap_or(""),
                                );
                                let st = ctx.text_edit.get_mut_or_default(focused_id);

                                let (start, end) = st
                                    .preedit
                                    .as_ref()
                                    .map(|preedit| preedit.range)
                                    .unwrap_or_else(|| st.selection_range());

                                if let Some(filtered_text) =
                                    Self::prepare_inserted_text(semantics, &value, start, end, text)
                                {
                                    let new_caret = start + filtered_text.len();
                                    let new_text = st.apply_edit(
                                        start..end,
                                        &filtered_text,
                                        new_caret,
                                        new_caret,
                                    );
                                    self.dispatch_change(ctx, semantics, focused_id, new_text);
                                    Self::dispatch_cursor_change(
                                        ctx, semantics, focused_id, new_caret, new_caret,
                                    );
                                } else {
                                    st.clear_preedit();
                                }

                                return true;
                            }
                        }
                    }
                }
            }
            crate::event::ImeEvent::Preedit { text } => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.disabled || semantics.read_only {
                                return true;
                            }
                        }
                    }
                    let st = ctx.text_edit.get_mut_or_default(focused_id);
                    st.set_preedit(text.clone());
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
        node_id: NodeId,
        new_text: String,
    ) {
        Self::persist_runtime_state(ctx, node_id);
        if let Some(action_entry) = semantics
            .actions
            .entries
            .iter()
            .find(|e| e.trigger == fission_ir::semantics::ActionTrigger::Change)
        {
            let payload = serde_json::to_vec(&new_text).unwrap();
            let envelope = ActionEnvelope {
                id: ActionId::from_u128(action_entry.action_id),
                payload,
            };
            ctx.dispatched_actions
                .push((node_id, envelope, crate::ActionInput::None));

            // State update moved to handle_key to avoid double borrow

            Self::auto_scroll_textinput(ctx, node_id);
        }
    }

    fn dispatch_cursor_change(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: NodeId,
        new_caret: usize,
        new_anchor: usize,
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

            let cursor_changed = crate::action::CursorChanged {
                caret: new_caret,
                anchor: new_anchor,
            };
            let payload = serde_json::to_vec(&cursor_changed).unwrap();
            let envelope = ActionEnvelope {
                id: ActionId::from_u128(action_entry.action_id),
                payload,
            };
            ctx.dispatched_actions
                .push((node_id, envelope, crate::ActionInput::None));
        }
    }

    fn dispatch_submit(
        ctx: &mut ControllerContext,
        semantics: &fission_ir::Semantics,
        node_id: NodeId,
        current_value: &str,
    ) -> bool {
        let mut dispatched = false;
        for trigger in [
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
        node_id: NodeId,
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
        ctx.dispatched_actions
            .push((node_id, envelope, crate::ActionInput::None));
        true
    }

    fn resolve_editing_value(
        ctx: &mut ControllerContext,
        focused_id: NodeId,
        semantic_value: &str,
    ) -> (String, usize, usize) {
        Self::sync_runtime_state(ctx, focused_id, semantic_value);
        let st = ctx.text_edit.get_mut_or_default(focused_id);
        let value = st.committed_text();
        (value, st.caret, st.anchor)
    }

    fn display_value_for_metrics(
        ctx: &mut ControllerContext,
        focused_id: NodeId,
        semantic_value: &str,
    ) -> String {
        Self::sync_runtime_state(ctx, focused_id, semantic_value);
        let state = ctx.text_edit.get_mut_or_default(focused_id);
        state.display_text().0
    }

    fn mask_text_for_metrics(text: &str) -> String {
        let mut masked = String::new();
        for _ in text.graphemes(true) {
            masked.push('•');
        }
        masked
    }

    fn masked_byte_offset_from_source(
        source: &str,
        masked: &str,
        source_byte_offset: usize,
    ) -> usize {
        let clamped = source_byte_offset.min(source.len());
        let grapheme_count = source[..clamped].graphemes(true).count();
        masked
            .grapheme_indices(true)
            .nth(grapheme_count)
            .map(|(idx, _)| idx)
            .unwrap_or(masked.len())
    }

    fn source_byte_offset_from_masked(
        source: &str,
        masked: &str,
        masked_byte_offset: usize,
    ) -> usize {
        let clamped = masked_byte_offset.min(masked.len());
        let grapheme_count = masked[..clamped].graphemes(true).count();
        source
            .grapheme_indices(true)
            .nth(grapheme_count)
            .map(|(idx, _)| idx)
            .unwrap_or(source.len())
    }

    fn clamp_caret_to_value(value: &str, caret: usize) -> usize {
        if caret > value.len() {
            value.len()
        } else {
            caret
        }
    }

    fn prev_grapheme_boundary(value: &str, idx: usize) -> usize {
        let mut last = 0;
        for (pos, _) in value.grapheme_indices(true) {
            if pos >= idx {
                break;
            }
            last = pos;
        }
        last
    }

    fn next_grapheme_boundary(value: &str, idx: usize) -> usize {
        for (pos, _) in value.grapheme_indices(true) {
            if pos > idx {
                return pos;
            }
        }
        value.len()
    }

    fn prev_word_boundary(value: &str, idx: usize) -> usize {
        let at = idx.min(value.len());
        let segments: Vec<(usize, &str)> = value.split_word_bound_indices().collect();
        for (start, segment) in segments.into_iter().rev() {
            let end = start + segment.len();
            if end > at {
                continue;
            }
            if segment.chars().any(|ch| ch.is_alphanumeric() || ch == '_') {
                return start;
            }
        }
        0
    }

    fn next_word_boundary(value: &str, idx: usize) -> usize {
        let at = idx.min(value.len());
        for (start, segment) in value.split_word_bound_indices() {
            let end = start + segment.len();
            if end <= at {
                continue;
            }
            if segment.chars().any(|ch| ch.is_alphanumeric() || ch == '_') {
                return end;
            }
        }
        value.len()
    }

    fn find_scroll_container_and_text_op(
        ir: &fission_ir::CoreIR,
        root: NodeId,
        multiline_semantics: bool,
    ) -> Option<(NodeId, NodeId, op::FlexDirection)> {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if let Some(n) = ir.nodes.get(&id) {
                if let Op::Layout(op::LayoutOp::Scroll { direction, .. }) = &n.op {
                    let matches_multiline_config = (multiline_semantics
                        && *direction == op::FlexDirection::Column)
                        || (!multiline_semantics && *direction == op::FlexDirection::Row);
                    if matches_multiline_config {
                        let mut q = vec![id]; // Start BFS from scroll node to find text
                        while let Some(cid) = q.pop() {
                            if let Some(cn) = ir.nodes.get(&cid) {
                                if matches!(
                                    cn.op,
                                    Op::Paint(fission_ir::PaintOp::DrawText { .. })
                                        | Op::Paint(fission_ir::PaintOp::DrawRichText { .. })
                                ) {
                                    return Some((id, cid, *direction));
                                }
                                for &gc in &cn.children {
                                    q.push(gc);
                                }
                            }
                        }
                        return None; // Should find text inside. For now, assume it's directly related.
                    }
                }
                for &c in &n.children {
                    stack.push(c);
                }
            }
        }
        None
    }

    /// Extract rich text runs from the TextInput's DrawRichText child.
    fn extract_rich_runs(
        ir: &fission_ir::CoreIR,
        semantics_id: NodeId,
    ) -> Option<Vec<fission_ir::op::TextRun>> {
        fn walk(
            ir: &fission_ir::CoreIR,
            node_id: NodeId,
            depth: usize,
        ) -> Option<Vec<fission_ir::op::TextRun>> {
            if depth > 20 {
                return None;
            }
            let node = ir.nodes.get(&node_id)?;
            match &node.op {
                Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) if !runs.is_empty() => {
                    Some(runs.clone())
                }
                _ => {
                    for child_id in &node.children {
                        if let Some(r) = walk(ir, *child_id, depth + 1) {
                            return Some(r);
                        }
                    }
                    None
                }
            }
        }
        walk(ir, semantics_id, 0)
    }

    /// Extract the font size from the TextInput's DrawRichText or DrawText child.
    fn extract_font_size(ir: &fission_ir::CoreIR, semantics_id: NodeId) -> Option<f32> {
        // Walk children of the semantics node to find a text paint op
        fn walk(ir: &fission_ir::CoreIR, node_id: NodeId, depth: usize) -> Option<f32> {
            if depth > 10 {
                return None;
            }
            let node = ir.nodes.get(&node_id)?;
            match &node.op {
                Op::Paint(fission_ir::PaintOp::DrawText { size, .. }) => Some(*size),
                Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) => {
                    runs.first().map(|r| r.style.font_size)
                }
                _ => {
                    for child_id in &node.children {
                        if let Some(sz) = walk(ir, *child_id, depth + 1) {
                            return Some(sz);
                        }
                    }
                    None
                }
            }
        }
        walk(ir, semantics_id, 0)
    }

    /// Shared hit-test logic for both PointerDown and PointerMove.
    ///
    /// Uses the rich-text layout path when styled runs are available, passing the
    /// same `available_width` that the renderer will use so both sides build (or
    /// look up) the same Parley `Layout`.  This ensures the Y-to-line and X-to-
    /// glyph mapping in hit-testing exactly matches the rendered text.
    fn hit_test_text(
        measurer: &std::sync::Arc<dyn fission_layout::TextMeasurer>,
        ir: &fission_ir::CoreIR,
        focused_id: NodeId,
        prefer_plain_text: bool,
        text: &str,
        scroll_geom: &fission_layout::LayoutNodeGeometry,
        local_x: f32,
        local_y: f32,
    ) -> usize {
        let viewport_width = if scroll_geom.rect.size.width > 0.0 {
            Some(scroll_geom.rect.size.width)
        } else {
            None
        };
        let render_width = viewport_width;
        let font_size = Self::extract_font_size(ir, focused_id).unwrap_or(13.0);
        let paragraph = Self::extract_paragraph_style(ir, focused_id).unwrap_or_default();

        if paragraph.text_align != TextAlign::Start {
            let line_metrics = measurer.get_line_metrics(text, font_size, render_width);
            if let (Some(width), Some(line)) = (
                viewport_width,
                Self::line_metric_for_local_y(&line_metrics, local_y),
            ) {
                let aligned_x =
                    local_x - Self::paragraph_line_x_offset(paragraph, width, line.width, false);
                return measurer.hit_test(text, font_size, render_width, aligned_x, local_y);
            }
        }

        if !prefer_plain_text {
            if let Some(runs) = Self::extract_rich_runs(ir, focused_id) {
                return measurer.hit_test_rich(&runs, render_width, local_x, local_y);
            }
        }
        measurer.hit_test(text, font_size, render_width, local_x, local_y)
    }

    fn caret_from_point_in_text_fallback(
        _value: &str,
        _font_size: f32,
        _viewport_x: f32,
        _viewport_w: f32,
        _content_w: f32,
        _scroll_offset: f32,
        _point_x: f32,
    ) -> usize {
        // Simplified fallback: always return 0 if no proper measurer is available.
        // In a real scenario, this would ideally not be hit in interactive UIs.
        0
    }

    fn auto_scroll_textinput(ctx: &mut ControllerContext, text_root: NodeId) {
        let font_size = Self::extract_font_size(ctx.ir, text_root).unwrap_or(16.0);
        if let Some(measurer) = ctx.measurer {
            // Need to get multiline status from semantics here
            let is_multiline = if let Some(node) = ctx.ir.nodes.get(&text_root) {
                if let Op::Semantics(sem) = &node.op {
                    sem.multiline
                } else {
                    false
                }
            } else {
                false
            };

            if let Some((scroll_id, _text_op_node_id, scroll_direction)) =
                Self::find_scroll_container_and_text_op(ctx.ir, text_root, is_multiline)
            {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_size = scroll_geom.rect.size;

                    let (current_text_value, metric_text, masked, scroll_padding) =
                        if let Some(node) = ctx.ir.nodes.get(&text_root) {
                            if let Op::Semantics(sem) = &node.op {
                                let display_value = Self::display_value_for_metrics(
                                    ctx,
                                    text_root,
                                    sem.value.as_deref().unwrap_or(""),
                                );
                                let metric_text = if sem.masked {
                                    Self::mask_text_for_metrics(&display_value)
                                } else {
                                    display_value.clone()
                                };
                                (
                                    display_value,
                                    metric_text,
                                    sem.masked,
                                    sem.scroll_padding.unwrap_or([2.0, 3.0, 2.0, 3.0]),
                                )
                            } else {
                                (String::new(), String::new(), false, [2.0, 3.0, 2.0, 3.0])
                            }
                        } else {
                            (String::new(), String::new(), false, [2.0, 3.0, 2.0, 3.0])
                        };

                    let current_caret_idx = if let Some(st) = ctx.text_edit.get(text_root) {
                        st.caret
                    } else {
                        0
                    };
                    let metric_caret_idx = if masked {
                        Self::masked_byte_offset_from_source(
                            &current_text_value,
                            &metric_text,
                            current_caret_idx,
                        )
                    } else {
                        current_caret_idx
                    };
                    let paragraph =
                        Self::extract_paragraph_style(ctx.ir, text_root).unwrap_or_default();
                    let measurer_width = if scroll_direction == op::FlexDirection::Column {
                        Some(viewport_size.width)
                    } else {
                        None
                    };

                    let (caret_x, caret_y) = measurer.get_caret_position(
                        &metric_text,
                        font_size,
                        measurer_width,
                        metric_caret_idx,
                    );

                    let mut offset = ctx.scroll.get_offset(scroll_id);

                    if scroll_direction == op::FlexDirection::Row {
                        // Handle horizontal scrolling for single-line text
                        let line_width = measurer
                            .get_line_metrics(&metric_text, font_size, None)
                            .first()
                            .map(|line| line.width)
                            .unwrap_or_else(|| measurer.measure(&metric_text, font_size, None).0);
                        let caret_left = caret_x
                            + Self::paragraph_line_x_offset(
                                paragraph,
                                viewport_size.width,
                                line_width,
                                false,
                            );
                        let caret_width = 2.0f32;
                        let caret_right = caret_left + caret_width;

                        let margin_left = scroll_padding[0].max(0.0);
                        let margin_right = scroll_padding[1].max(0.0);

                        let visible_left = caret_left - offset;
                        let visible_right = caret_right - offset;

                        if visible_right > (viewport_size.width - margin_right) {
                            offset =
                                (caret_right - (viewport_size.width - margin_right)).max(0.0f32);
                        } else if visible_left < margin_left {
                            offset = (caret_left - margin_left).max(0.0f32);
                        }
                        let content_w = scroll_geom.content_size.width.max(viewport_size.width);
                        let max_offset = (content_w - viewport_size.width).max(0.0f32);
                        offset = offset.clamp(0.0f32, max_offset);
                        ctx.scroll.set_offset(scroll_id, offset);
                    } else {
                        // op::FlexDirection::Column
                        // Handle vertical scrolling for multi-line text
                        let caret_top = caret_y;
                        let caret_height = measurer
                            .measure("Tg", font_size, Some(viewport_size.width))
                            .1;
                        let caret_bottom = caret_top + caret_height;

                        let margin_top = scroll_padding[2].max(0.0);
                        let margin_bottom = scroll_padding[3].max(0.0);

                        let visible_top = caret_top - offset;
                        let visible_bottom = caret_bottom - offset;

                        if visible_bottom > (viewport_size.height - margin_bottom) {
                            offset =
                                (caret_bottom - (viewport_size.height - margin_bottom)).max(0.0f32);
                        } else if visible_top < margin_top {
                            offset = (caret_top - margin_top).max(0.0f32);
                        }
                        let content_h = scroll_geom.content_size.height.max(viewport_size.height);
                        let max_offset = (content_h - viewport_size.height).max(0.0f32);
                        offset = offset.clamp(0.0f32, max_offset);
                        ctx.scroll.set_offset(scroll_id, offset);
                    }
                }
            }
        }
    }

    fn handle_vertical_navigation(
        &mut self,
        ctx: &mut ControllerContext,
        focused_id: NodeId,
        semantics: &Semantics,
        value: &str,
        caret: usize,
        modifiers: u8,
        is_up: bool,
    ) {
        if let Some(measurer) = ctx.measurer {
            if let Some((scroll_id, _text_op_node_id, _scroll_direction)) =
                Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
            {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_w = scroll_geom.rect.size.width;
                    let font_size = Self::extract_font_size(ctx.ir, focused_id).unwrap_or(16.0);

                    let (current_caret_x, _current_caret_y) =
                        measurer.get_caret_position(value, font_size, Some(viewport_w), caret);

                    let line_metrics =
                        measurer.get_line_metrics(value, font_size, Some(viewport_w));

                    let mut current_line_idx = 0;
                    for (idx, line) in line_metrics.iter().enumerate() {
                        if caret >= line.start_index && caret <= line.end_index {
                            current_line_idx = idx;
                            // Don't break: if the caret sits at the boundary
                            // between two lines (end of line N == start of
                            // line N+1), prefer the later line so empty lines
                            // are reachable.
                        }
                    }

                    let target_line_idx = if is_up {
                        current_line_idx.saturating_sub(1)
                    } else {
                        (current_line_idx + 1).min(line_metrics.len().saturating_sub(1))
                    };

                    if let Some(target_line) = line_metrics.get(target_line_idx) {
                        let target_y = target_line.baseline;

                        let mut new_caret_pos = measurer.hit_test(
                            value,
                            font_size,
                            Some(viewport_w),
                            current_caret_x,
                            target_y,
                        );

                        // Ensure we stay within the target line's bounds.
                        // For empty lines (start_index == end_index), this
                        // correctly places the cursor at start_index.
                        new_caret_pos = new_caret_pos.clamp(
                            target_line.start_index,
                            target_line.end_index.max(target_line.start_index),
                        );

                        let st = ctx.text_edit.get_mut_or_default(focused_id);
                        st.caret = new_caret_pos;
                        if !Self::has_shift(modifiers) {
                            st.anchor = new_caret_pos;
                        } // If no shift, collapse selection
                        let final_anchor = st.anchor;
                        Self::auto_scroll_textinput(ctx, focused_id);
                        Self::dispatch_cursor_change(
                            ctx,
                            semantics,
                            focused_id,
                            new_caret_pos,
                            final_anchor,
                        );
                    }
                }
            }
        }
    }

    fn handle_page_navigation(
        &mut self,
        ctx: &mut ControllerContext,
        focused_id: NodeId,
        semantics: &Semantics,
        value: &str,
        caret: usize,
        modifiers: u8,
        is_page_up: bool,
    ) {
        if let Some(measurer) = ctx.measurer {
            if let Some((scroll_id, _text_op_node_id, _scroll_direction)) =
                Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
            {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_w = scroll_geom.rect.size.width;
                    let viewport_h = scroll_geom.rect.size.height.max(1.0);
                    let font_size = Self::extract_font_size(ctx.ir, focused_id).unwrap_or(16.0);
                    let (current_caret_x, _current_caret_y) =
                        measurer.get_caret_position(value, font_size, Some(viewport_w), caret);
                    let line_metrics =
                        measurer.get_line_metrics(value, font_size, Some(viewport_w));

                    if line_metrics.is_empty() {
                        return;
                    }

                    let mut current_line_idx = 0usize;
                    for (idx, line) in line_metrics.iter().enumerate() {
                        if caret >= line.start_index && caret <= line.end_index {
                            current_line_idx = idx;
                        }
                    }

                    let line_height = line_metrics
                        .get(current_line_idx)
                        .map(|line| line.height.max(1.0))
                        .unwrap_or(20.0);
                    let lines_per_page = (viewport_h / line_height).floor().max(1.0) as isize;
                    let delta = if is_page_up {
                        -lines_per_page
                    } else {
                        lines_per_page
                    };
                    let target_line_idx = current_line_idx
                        .saturating_add_signed(delta)
                        .min(line_metrics.len().saturating_sub(1));

                    if let Some(target_line) = line_metrics.get(target_line_idx) {
                        let target_y = target_line.baseline;
                        let mut new_caret_pos = measurer.hit_test(
                            value,
                            font_size,
                            Some(viewport_w),
                            current_caret_x,
                            target_y,
                        );
                        let target_end = Self::trim_line_end(
                            value,
                            target_line.end_index.max(target_line.start_index),
                        );
                        new_caret_pos = new_caret_pos.clamp(
                            target_line.start_index,
                            target_end.max(target_line.start_index),
                        );

                        let st = ctx.text_edit.get_mut_or_default(focused_id);
                        st.caret = new_caret_pos;
                        if !Self::has_shift(modifiers) {
                            st.anchor = new_caret_pos;
                        }
                        let final_anchor = st.anchor;
                        Self::auto_scroll_textinput(ctx, focused_id);
                        Self::dispatch_cursor_change(
                            ctx,
                            semantics,
                            focused_id,
                            new_caret_pos,
                            final_anchor,
                        );
                    }
                }
            }
        }
    }

    fn extract_paragraph_style(
        ir: &fission_ir::CoreIR,
        semantics_id: NodeId,
    ) -> Option<TextParagraphStyle> {
        fn walk(
            ir: &fission_ir::CoreIR,
            node_id: NodeId,
            depth: usize,
        ) -> Option<TextParagraphStyle> {
            if depth > 10 {
                return None;
            }
            let node = ir.nodes.get(&node_id)?;
            match &node.op {
                Op::Paint(fission_ir::PaintOp::DrawText {
                    paragraph_style,
                    caret_width,
                    ..
                }) => paragraph_style.or_else(|| decode_text_paragraph_style(*caret_width)),
                Op::Paint(fission_ir::PaintOp::DrawRichText {
                    paragraph_style,
                    caret_width,
                    ..
                }) => paragraph_style.or_else(|| decode_text_paragraph_style(*caret_width)),
                _ => {
                    for child_id in &node.children {
                        if let Some(style) = walk(ir, *child_id, depth + 1) {
                            return Some(style);
                        }
                    }
                    None
                }
            }
        }
        walk(ir, semantics_id, 0)
    }

    fn line_metric_for_local_y<'a>(
        line_metrics: &'a [fission_layout::LineMetric],
        local_y: f32,
    ) -> Option<&'a fission_layout::LineMetric> {
        if line_metrics.is_empty() {
            return None;
        }
        let mut line_top = 0.0f32;
        for (index, line) in line_metrics.iter().enumerate() {
            let line_height = line.height.max(1.0);
            let line_bottom = line_top + line_height;
            if local_y < line_bottom || index + 1 == line_metrics.len() {
                return Some(line);
            }
            line_top = line_bottom;
        }
        line_metrics.last()
    }

    fn paragraph_line_x_offset(
        paragraph: TextParagraphStyle,
        bounds_width: f32,
        line_width: f32,
        is_last_line: bool,
    ) -> f32 {
        if bounds_width <= 0.0 {
            return 0.0;
        }

        match paragraph.text_align {
            TextAlign::Start | TextAlign::Left => 0.0,
            TextAlign::Center => (bounds_width - line_width) * 0.5,
            TextAlign::End | TextAlign::Right => bounds_width - line_width,
            TextAlign::Justify if is_last_line => 0.0,
            TextAlign::Justify => 0.0,
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
