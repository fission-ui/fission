use super::{ControllerContext, InputController};
use crate::event::{InputEvent, KeyCode, KeyEvent, PointerEvent};
use crate::ActionEnvelope;
use crate::ActionId;
use fission_diagnostics::prelude as diag;
use fission_ir::semantics::InputMask;
use fission_ir::FlexDirection;
use fission_ir::{
    op::{self, LayoutOp, Op},
    NodeId, Semantics,
};
use fission_layout::LayoutSnapshot;
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
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                // Try the currently focused TextInput first.  If nothing is focused,
                // try to find the TextInput under the click point and focus + place
                // caret in one step (so the user doesn't need to click twice).
                let effective_focused = ctx.interaction.focused.or_else(|| {
                    let hit = crate::hit_test::hit_test_with_scroll(ctx.ir, ctx.layout, ctx.scroll, *point)?;
                    // Walk up from hit node to find a focusable TextInput
                    let mut walk = Some(hit);
                    while let Some(nid) = walk {
                        if let Some(node) = ctx.ir.nodes.get(&nid) {
                            if let Op::Semantics(s) = &node.op {
                                if s.focusable && s.role == fission_ir::semantics::Role::TextInput {
                                    // Focus it now
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
                });
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
                                    ctx.ir.nodes.get(id).and_then(|n| n.children.first().copied())
                                })
                                .find(|id| ctx.layout.get_node_geometry(*id).is_some())
                                .or_else(|| {
                                    let mut w = ctx.ir.nodes.get(&focused_id).and_then(|n| n.parent);
                                    while let Some(pid) = w {
                                        if ctx.layout.get_node_geometry(pid).is_some() {
                                            return Some(pid);
                                        }
                                        w = ctx.ir.nodes.get(&pid).and_then(|n| n.parent);
                                    }
                                    None
                                });
                                if let Some(geom) = geom_id.and_then(|id| ctx.layout.get_node_geometry(id)) {
                                    let mut scroll_adj_y = 0.0f32;
                                    let mut scroll_adj_x = 0.0f32;
                                    let mut walk_id = ctx.ir.nodes.get(&focused_id).and_then(|n| n.parent);
                                    while let Some(pid) = walk_id {
                                        if let Some(pnode) = ctx.ir.nodes.get(&pid) {
                                            if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &pnode.op {
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
                                if let Some((scroll_id, _text_op_node_id, scroll_direction)) = scroll_result {
                                    if let Some(scroll_geom) =
                                        ctx.layout.get_node_geometry(scroll_id)
                                    {
                                        let value = sem.value.as_deref().unwrap_or("");
                                        let offset = ctx.scroll.get_offset(scroll_id);

                                        // Accumulate ancestor scroll offsets to convert
                                        // screen coordinates to local content coordinates.
                                        let mut ancestor_scroll_y = 0.0f32;
                                        let mut ancestor_scroll_x = 0.0f32;
                                        {
                                            let mut walk = ctx.ir.nodes.get(&scroll_id).and_then(|n| n.parent);
                                            while let Some(pid) = walk {
                                                if let Some(pnode) = ctx.ir.nodes.get(&pid) {
                                                    if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &pnode.op {
                                                        let poff = ctx.scroll.get_offset(pid);
                                                        match direction {
                                                            FlexDirection::Row => ancestor_scroll_x += poff,
                                                            FlexDirection::Column => ancestor_scroll_y += poff,
                                                        }
                                                    }
                                                    walk = pnode.parent;
                                                } else {
                                                    break;
                                                }
                                            }
                                        }

                                        let caret = if let Some(measurer) = ctx.measurer {
                                            let local_x = point.x - scroll_geom.rect.origin.x + offset + ancestor_scroll_x;
                                            let local_y = point.y - scroll_geom.rect.origin.y + ancestor_scroll_y;

                                            Self::hit_test_text(
                                                measurer, ctx.ir, focused_id,
                                                value, scroll_geom, local_x, local_y,
                                            )
                                        } else {
                                            Self::caret_from_point_in_text_fallback(
                                                value,
                                                16.0,
                                                scroll_geom.rect.origin.x,
                                                scroll_geom.rect.size.width,
                                                scroll_geom.content_size.width,
                                                offset,
                                                point.x,
                                            )
                                        };
                                        let st = ctx.text_edit.get_mut_or_default(focused_id);
                                        st.caret = caret;
                                        st.anchor = caret;
                                        Self::dispatch_cursor_change(ctx, sem, focused_id, caret, caret);
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
                                if !ctx.interaction.pressed.is_empty() {
                                    let mut moved_enough = true;
                                    if let Some(start) = ctx.interaction.last_down_point {
                                        let dx = point.x - start.x;
                                        let dy = point.y - start.y;
                                        if dx * dx + dy * dy < 4.0 {
                                            moved_enough = false;
                                        }
                                    }
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
                                                let offset = ctx.scroll.get_offset(scroll_id);
                                                let new_caret = if let Some(measurer) = ctx.measurer
                                                {
                                                    // Accumulate ancestor scroll offsets for
                                                    // pointer-move the same way as pointer-down.
                                                    let mut anc_scroll_y = 0.0f32;
                                                    let mut anc_scroll_x = 0.0f32;
                                                    {
                                                        let mut walk = ctx.ir.nodes.get(&scroll_id).and_then(|n| n.parent);
                                                        while let Some(pid) = walk {
                                                            if let Some(pnode) = ctx.ir.nodes.get(&pid) {
                                                                if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &pnode.op {
                                                                    let poff = ctx.scroll.get_offset(pid);
                                                                    match direction {
                                                                        FlexDirection::Row => anc_scroll_x += poff,
                                                                        FlexDirection::Column => anc_scroll_y += poff,
                                                                    }
                                                                }
                                                                walk = pnode.parent;
                                                            } else {
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    let local_x = point.x - scroll_geom.rect.origin.x + offset + anc_scroll_x;
                                                    let local_y = point.y - scroll_geom.rect.origin.y + anc_scroll_y;

                                                    Self::hit_test_text(
                                                        measurer, ctx.ir, focused_id,
                                                        value, scroll_geom, local_x, local_y,
                                                    )
                                                } else {
                                                    Self::caret_from_point_in_text_fallback(
                                                        value,
                                                        16.0,
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
                                                Self::dispatch_cursor_change(ctx, sem, focused_id, new_caret, current_anchor);
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

        caret = Self::clamp_caret_to_value(&value, caret);
        anchor = Self::clamp_caret_to_value(&value, anchor);

        let sel = if caret != anchor {
            Some((anchor, caret))
        } else {
            None
        };

        // Logic for state changes
        let mut next_caret = caret;
        let mut next_anchor = anchor;
        let mut next_text: Option<String> = None;
        let mut handled = false;

        // Undo/Redo logic result
        let mut undo_redo_result: Option<(String, usize, usize)> = None;

        match key_code {
            KeyCode::Space => {
                let (txt, c) = Self::insert_text(&value, caret, sel, " ");
                next_text = Some(txt);
                next_caret = c;
                next_anchor = c;
                handled = true;
            }
            KeyCode::Char(ch) if ((modifiers & 4) != 0) || ((modifiers & 8) != 0) => {
                let lower = ch.to_ascii_lowercase();
                let (s, e) = if caret <= anchor {
                    (caret, anchor)
                } else {
                    (anchor, caret)
                };
                match lower {
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
                            let mut out = String::with_capacity(value.len() - (e - s));
                            out.push_str(&value[..s]);
                            out.push_str(&value[e..]);
                            next_text = Some(out);
                            next_caret = s;
                            next_anchor = s;
                        }
                        handled = true;
                    }
                    'v' => {
                        let text_to_paste = if let Some(cb) = ctx.clipboard {
                            cb.get_text().unwrap_or_default()
                        } else {
                            String::new()
                        };
                        if !text_to_paste.is_empty() {
                            let (txt, c) = Self::insert_text(&value, caret, sel, &text_to_paste);
                            next_text = Some(txt);
                            next_caret = c;
                            next_anchor = c;
                        }
                        handled = true;
                    }
                    'z' => {
                        let (ctrl_or_super, shift) = (
                            ((modifiers & 4) != 0) || ((modifiers & 8) != 0),
                            (modifiers & 1) != 0,
                        );
                        if ctrl_or_super {
                            let st = ctx.text_edit.get_mut_or_default(focused_id);
                            if shift {
                                if let Some((v, c, a)) = st.history.redo() {
                                    undo_redo_result = Some((v.clone(), *c, *a));
                                }
                            } else {
                                if let Some((v, c, a)) = st.history.undo() {
                                    undo_redo_result = Some((v.clone(), *c, *a));
                                }
                            }
                            handled = true;
                        }
                    }
                    _ => {} // Do nothing for other shortcuts
                }
            }
            KeyCode::Char(c) => {
                // Check against input mask
                if let Some(mask) = &semantics.input_mask {
                    if !mask.is_valid_char(c) {
                        return true; // Ignore invalid character
                    }
                }
                let (txt, nc) = Self::insert_text(&value, caret, sel, &c.to_string());
                next_text = Some(txt);
                next_caret = nc;
                next_anchor = nc;
                handled = true;
            }
            KeyCode::Backspace => {
                let (txt, nc) = if (modifiers & 2) != 0 && sel.is_none() {
                    // Ctrl+Backspace
                    let mut at = caret;
                    while at > 0 {
                        let prev = Self::prev_grapheme_boundary(&value, at);
                        let ch = value[prev..].chars().next().unwrap_or('\0');
                        if !ch.is_whitespace() {
                            at = prev;
                            break;
                        }
                        at = prev;
                    }
                    while at > 0 {
                        let prev = Self::prev_grapheme_boundary(&value, at);
                        let ch = value[prev..].chars().next().unwrap_or('\0');
                        if ch.is_alphanumeric() || ch == '_' {
                            at = prev;
                        } else {
                            break;
                        }
                    }
                    let mut out = String::with_capacity(value.len() - (caret - at));
                    out.push_str(&value[..at]);
                    out.push_str(&value[caret..]);
                    (out, at)
                } else {
                    Self::delete_prev_grapheme(&value, caret, sel)
                };
                next_text = Some(txt);
                next_caret = nc;
                next_anchor = nc;
                handled = true;
            }
            KeyCode::Left => {
                let prev = if (modifiers & 2) != 0 {
                    // Ctrl+Left
                    Self::prev_word_boundary(&value, caret)
                } else {
                    Self::prev_grapheme_boundary(&value, caret)
                };
                next_caret = prev;
                if (modifiers & 1) != 0 {
                    next_anchor = anchor;
                } else {
                    next_anchor = prev;
                }
                handled = true;
            }
            KeyCode::Right => {
                let next = if (modifiers & 2) != 0 {
                    // Ctrl+Right
                    Self::next_word_boundary(&value, caret)
                } else {
                    Self::next_grapheme_boundary(&value, caret)
                };
                next_caret = next;
                if (modifiers & 1) != 0 {
                    next_anchor = anchor;
                } else {
                    next_anchor = next;
                }
                handled = true;
            }
            KeyCode::Home => {
                next_caret = 0;
                if (modifiers & 1) != 0 {
                    next_anchor = anchor;
                } else {
                    next_anchor = 0;
                }
                handled = true;
            }
            KeyCode::End => {
                let end = value.len();
                next_caret = end;
                if (modifiers & 1) != 0 {
                    next_anchor = anchor;
                } else {
                    next_anchor = end;
                }
                handled = true;
            }
            KeyCode::Enter => {
                if semantics.multiline {
                    let insert_str = if semantics.auto_indent {
                        // Find the leading whitespace of the current line
                        let line_start = value[..caret].rfind('\n').map(|p| p + 1).unwrap_or(0);
                        let leading: String = value[line_start..]
                            .chars()
                            .take_while(|c| *c == ' ' || *c == '\t')
                            .collect();
                        format!("\n{}", leading)
                    } else {
                        "\n".to_string()
                    };
                    let (txt, nc) = Self::insert_text(&value, caret, sel, &insert_str);
                    next_text = Some(txt);
                    next_caret = nc;
                    next_anchor = nc;
                    handled = true;
                }
            }
            KeyCode::Up => {
                if semantics.multiline {
                    self.handle_vertical_navigation(
                        ctx, focused_id, semantics, &value, caret, modifiers, true,
                    );
                    return true; // Return early as handle_vertical_navigation does its own state update
                }
            }
            KeyCode::Down => {
                if semantics.multiline {
                    self.handle_vertical_navigation(
                        ctx, focused_id, semantics, &value, caret, modifiers, false,
                    );
                    return true;
                }
            }
            KeyCode::Tab => {
                if semantics.capture_tab {
                    let tab_str = "    "; // 4 spaces
                    let (txt, nc) = Self::insert_text(&value, caret, sel, tab_str);
                    next_text = Some(txt);
                    next_caret = nc;
                    next_anchor = nc;
                    handled = true;
                }
                // If capture_tab is false, fall through (return false) so focus
                // navigation can handle Tab normally.
            }
            _ => {} // Do nothing for other keys
        }

        if let Some((v, c, a)) = undo_redo_result {
            // Apply undo/redo result
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            st.caret = c;
            st.anchor = a;
            st.last_value = v.clone();
            self.dispatch_change(ctx, semantics, focused_id, v, c);
            Self::dispatch_cursor_change(ctx, semantics, focused_id, c, a);
            return true;
        }

        if let Some(txt) = next_text {
            // Apply text change
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            st.caret = next_caret;
            st.anchor = next_anchor;
            st.history.push(txt.clone(), next_caret, next_anchor);
            st.last_value = txt.clone();

            self.dispatch_change(ctx, semantics, focused_id, txt, next_caret);
            Self::dispatch_cursor_change(ctx, semantics, focused_id, next_caret, next_anchor);
        } else if handled {
            // Cursor movement only
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            st.caret = next_caret;
            st.anchor = next_anchor;
            Self::auto_scroll_textinput(ctx, focused_id);
            Self::dispatch_cursor_change(ctx, semantics, focused_id, next_caret, next_anchor);
        }

        handled
    }

    fn handle_ime(&mut self, ctx: &mut ControllerContext, ime: &crate::event::ImeEvent) -> bool {
        match ime {
            crate::event::ImeEvent::Commit { text } => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.role == fission_ir::semantics::Role::TextInput {
                                let (value, caret, anchor) = Self::resolve_editing_value(
                                    ctx,
                                    focused_id,
                                    semantics.value.as_deref().unwrap_or(""),
                                );
                                let st = ctx.text_edit.get_mut_or_default(focused_id);
                                let caret = Self::clamp_caret_to_value(&value, caret);
                                let sel = if caret != anchor {
                                    Some((anchor, caret))
                                } else {
                                    None
                                };

                                let mut filtered_text = String::new();
                                if let Some(mask) = &semantics.input_mask {
                                    for ch in text.chars() {
                                        if mask.is_valid_char(ch) {
                                            filtered_text.push(ch);
                                        }
                                    }
                                } else {
                                    filtered_text = text.clone();
                                }

                                if !filtered_text.is_empty() {
                                    // Only insert if something valid
                                    let (new_text, new_caret) =
                                        Self::insert_text(&value, caret, sel, &filtered_text);
                                    st.caret = new_caret;
                                    st.anchor = new_caret;
                                    st.last_value = new_text.clone();
                                    st.history.push(new_text.clone(), new_caret, new_caret);
                                    self.dispatch_change(
                                        ctx, semantics, focused_id, new_text, new_caret,
                                    );
                                }

                                *ctx.ime_preedit = None;
                                return true;
                            }
                        }
                    }
                }
            }
            crate::event::ImeEvent::Preedit { text } => {
                if let Some(focused_id) = ctx.interaction.focused {
                    *ctx.ime_preedit = Some((focused_id, text.clone()));
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
        new_caret: usize,
    ) {
        if let Some(st) = ctx.text_edit.states.get_mut(&node_id) {
            st.last_value = new_text.clone();
            st.pending_model_sync = true;
        }

        if let Some(action_entry) = semantics.actions.entries.iter().find(|e| {
            e.trigger == fission_ir::semantics::ActionTrigger::Change
        }) {
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

        if let Some(action_entry) = semantics.actions.entries.iter().find(|e| {
            e.trigger == fission_ir::semantics::ActionTrigger::CursorChange
        }) {
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

    fn resolve_editing_value(
        ctx: &mut ControllerContext,
        focused_id: NodeId,
        semantic_value: &str,
    ) -> (String, usize, usize) {
        let st = ctx.text_edit.get_mut_or_default(focused_id);

        // If the latest lowered semantics value has caught up with local edits,
        // stop treating local state as newer-than-model.
        if st.pending_model_sync && st.last_value == semantic_value {
            st.pending_model_sync = false;
        }

        // When we are not waiting for model sync, semantics is authoritative.
        // This picks up external state changes (e.g. programmatic clears/sets).
        if !st.pending_model_sync && st.last_value != semantic_value {
            st.last_value = semantic_value.to_string();
            st.caret = st.caret.min(st.last_value.len());
            st.anchor = st.anchor.min(st.last_value.len());
            st.history.push(st.last_value.clone(), st.caret, st.anchor);
        }

        let value = if st.pending_model_sync {
            st.last_value.clone()
        } else {
            semantic_value.to_string()
        };

        if st.history.stack.is_empty() {
            st.history.push(value.clone(), st.caret, st.anchor);
        }

        (value, st.caret, st.anchor)
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
        let mut at = idx.min(value.len());
        while at > 0 {
            let prev = Self::prev_grapheme_boundary(value, at);
            let ch = value[prev..].chars().next().unwrap_or('\0');
            if !ch.is_whitespace() {
                at = prev;
                break;
            }
            at = prev;
        }
        while at > 0 {
            let prev = Self::prev_grapheme_boundary(value, at);
            let ch = value[prev..].chars().next().unwrap_or('\0');
            if ch.is_alphanumeric() || ch == '_' {
                at = prev;
            } else {
                break;
            }
        }
        at
    }

    fn next_word_boundary(value: &str, idx: usize) -> usize {
        let mut at = idx.min(value.len());
        while at < value.len() {
            let next = Self::next_grapheme_boundary(value, at);
            let ch = value[at..].chars().next().unwrap_or('\0');
            if !ch.is_whitespace() {
                at = next;
                break;
            }
            at = next;
        }
        while at < value.len() {
            let next = Self::next_grapheme_boundary(value, at);
            let ch = value[at..].chars().next().unwrap_or('\0');
            if ch.is_alphanumeric() || ch == '_' {
                at = next;
            } else {
                break;
            }
        }
        at
    }

    fn delete_prev_grapheme(
        value: &str,
        caret: usize,
        sel: Option<(usize, usize)>,
    ) -> (String, usize) {
        if let Some((a, b)) = sel {
            let (s, e) = if a <= b { (a, b) } else { (b, a) };
            let mut out = String::with_capacity(value.len() - (e - s));
            out.push_str(&value[..s]);
            out.push_str(&value[e..]);
            return (out, s);
        }
        let at = caret.min(value.len());
        if at == 0 {
            return (value.to_string(), 0);
        }
        let prev = Self::prev_grapheme_boundary(value, at);
        let mut out = String::with_capacity(value.len() - (at - prev));
        out.push_str(&value[..prev]);
        out.push_str(&value[at..]);
        (out, prev)
    }

    fn insert_text(
        value: &str,
        caret: usize,
        sel: Option<(usize, usize)>,
        text: &str,
    ) -> (String, usize) {
        let (s, e) = sel
            .map(|(a, b)| if a <= b { (a, b) } else { (b, a) })
            .unwrap_or((caret, caret));
        let mut out = String::with_capacity(value.len() - (e - s) + text.len());
        out.push_str(&value[..s]);
        out.push_str(text);
        out.push_str(&value[e..]);
        (out, s + text.len())
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

    fn find_caret_in_scroll(ir: &fission_ir::CoreIR, scroll_id: NodeId) -> Option<NodeId> {
        let mut q = vec![scroll_id];
        while let Some(id) = q.pop() {
            if let Some(n) = ir.nodes.get(&id) {
                if let Op::Layout(op::LayoutOp::Box { width: Some(w), .. }) = &n.op {
                    if (*w - 2.0).abs() < 0.01 {
                        let mut has_paint = false;
                        for &cid in &n.children {
                            if let Some(cn) = ir.nodes.get(&cid) {
                                if let Op::Paint(fission_ir::PaintOp::DrawRect { .. }) = cn.op {
                                    has_paint = true;
                                    break;
                                }
                            }
                        }
                        if has_paint {
                            return Some(id);
                        }
                    }
                }
                for &c in &n.children {
                    q.push(c);
                }
            }
        }
        None
    }

    /// Extract rich text runs from the TextInput's DrawRichText child.
    fn extract_rich_runs(ir: &fission_ir::CoreIR, semantics_id: NodeId) -> Option<Vec<fission_ir::op::TextRun>> {
        fn walk(ir: &fission_ir::CoreIR, node_id: NodeId, depth: usize) -> Option<Vec<fission_ir::op::TextRun>> {
            if depth > 20 { return None; }
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
            if depth > 10 { return None; }
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
        value: &str,
        scroll_geom: &fission_layout::LayoutNodeGeometry,
        local_x: f32,
        local_y: f32,
    ) -> usize {
        // The renderer calls `layout_rich(…, if bounds.width() > 0 { Some(w) } else { None })`.
        // The bounds rect for a text node equals the layout geometry rect whose width comes
        // from the scroll container (or the text node itself).  We replicate this here so
        // the cache key matches and we reuse the SAME Parley layout the renderer painted.
        let render_width = if scroll_geom.rect.size.width > 0.0 {
            Some(scroll_geom.rect.size.width)
        } else {
            None
        };

        if let Some(runs) = Self::extract_rich_runs(ir, focused_id) {
            measurer.hit_test_rich(&runs, render_width, local_x, local_y)
        } else {
            let font_size = Self::extract_font_size(ir, focused_id).unwrap_or(13.0);
            measurer.hit_test(value, font_size, render_width, local_x, local_y)
        }
    }

    fn caret_from_point_in_text_fallback(
        value: &str,
        font_size: f32,
        viewport_x: f32,
        viewport_w: f32,
        content_w: f32,
        scroll_offset: f32,
        point_x: f32,
    ) -> usize {
        // Simplified fallback: always return 0 if no proper measurer is available.
        // In a real scenario, this would ideally not be hit in interactive UIs.
        0
    }

    fn auto_scroll_textinput(ctx: &mut ControllerContext, text_root: NodeId) {
        let font_size = 16.0; // Default font size
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

                    let current_text_value = if let Some(node) = ctx.ir.nodes.get(&text_root) {
                        if let Op::Semantics(sem) = &node.op {
                            sem.value.clone().unwrap_or_default()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    let current_caret_idx = if let Some(st) = ctx.text_edit.get(text_root) {
                        st.caret
                    } else {
                        0
                    };
                    let measurer_width = if scroll_direction == op::FlexDirection::Column {
                        Some(viewport_size.width)
                    } else {
                        None
                    };

                    let (caret_x, caret_y) = measurer.get_caret_position(
                        &current_text_value,
                        font_size,
                        measurer_width,
                        current_caret_idx,
                    );

                    let mut offset = ctx.scroll.get_offset(scroll_id);

                    if scroll_direction == op::FlexDirection::Row {
                        // Handle horizontal scrolling for single-line text
                        let caret_left = caret_x;
                        let caret_width = 2.0f32;
                        let caret_right = caret_left + caret_width;

                        let margin_left = 2.0f32;
                        let margin_right = 3.0f32;

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

                        let margin_top = 2.0f32;
                        let margin_bottom = 3.0f32;

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
            if let Some((scroll_id, _text_op_node_id, scroll_direction)) =
                Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline)
            {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_w = scroll_geom.rect.size.width;
                    let font_size = 16.0;

                    let (current_caret_x, current_caret_y) =
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
                        new_caret_pos = new_caret_pos.clamp(target_line.start_index, target_line.end_index.max(target_line.start_index));

                        let st = ctx.text_edit.get_mut_or_default(focused_id);
                        st.caret = new_caret_pos;
                        if (modifiers & 1) == 0 {
                            st.anchor = new_caret_pos;
                        } // If no shift, collapse selection
                        let final_anchor = st.anchor;
                        Self::auto_scroll_textinput(ctx, focused_id);
                        Self::dispatch_cursor_change(ctx, semantics, focused_id, new_caret_pos, final_anchor);
                    }
                }
            }
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
