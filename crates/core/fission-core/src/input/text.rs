use super::{ControllerContext, InputController};
use crate::event::{InputEvent, KeyCode, KeyEvent, PointerEvent};
use crate::ActionEnvelope;
use crate::ActionId;
use fission_diagnostics::prelude as diag;
use fission_ir::{op::{self, LayoutOp, Op}, NodeId, Semantics};
use fission_layout::LayoutSnapshot;
use serde_json;
use unicode_segmentation::UnicodeSegmentation;

pub struct TextInputController;

impl InputController for TextInputController {
    fn handle_event(&mut self, ctx: &mut ControllerContext, event: &InputEvent) -> bool {
        match event {
            InputEvent::Keyboard(KeyEvent::Down { key_code, modifiers }) => {
                self.handle_key(ctx, key_code.clone(), *modifiers)
            }
            InputEvent::Ime(ime) => {
                self.handle_ime(ctx, ime)
            }
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(sem) = &node.op {
                            if sem.role == fission_ir::semantics::Role::TextInput {
                                if let Some((scroll_id, _text_op_node_id, scroll_direction)) = Self::find_scroll_container_and_text_op(ctx.ir, focused_id, sem.multiline) {
                                    if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                                        let value = sem.value.as_deref().unwrap_or("");
                                        let offset = ctx.scroll.get_offset(scroll_id);

                                        let caret = if let Some(measurer) = ctx.measurer {
                                            let font_size = 16.0;
                                            let max_width = if scroll_geom.rect.width() > 0.0 { Some(scroll_geom.rect.width()) } else { None };
                                            measurer.hit_test(value, font_size, max_width, point.x - scroll_geom.rect.origin.x + offset, point.y - scroll_geom.rect.origin.y)
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
                                        if dx*dx + dy*dy < 4.0 { moved_enough = false; }
                                    }
                                    if moved_enough {
                                        if let Some((scroll_id, _text_op_node_id, scroll_direction)) = Self::find_scroll_container_and_text_op(ctx.ir, focused_id, sem.multiline) {
                                            if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                                                let value = sem.value.as_deref().unwrap_or("");
                                                let offset = ctx.scroll.get_offset(scroll_id);
                                                let new_caret = if let Some(measurer) = ctx.measurer {
                                                    let font_size = 16.0;
                                                    let max_width = if scroll_geom.rect.width() > 0.0 { Some(scroll_geom.rect.width()) } else { None };
                                                    measurer.hit_test(value, font_size, max_width, point.x - scroll_geom.rect.origin.x + offset, point.y - scroll_geom.rect.origin.y)
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
                                                st.caret = new_caret; 
                                                Self::auto_scroll_textinput(ctx, focused_id);
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
            _ => false
        }
    }
}

impl TextInputController {
    fn handle_key(&mut self, ctx: &mut ControllerContext, key_code: KeyCode, modifiers: u8) -> bool {
        let focused_id = if let Some(id) = ctx.interaction.focused { id } else { return false; };
        
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
            } else { break; }
        }
        
        let semantics = if let Some(s) = semantics_node { s } else { return false; };
        
        let value = semantics.value.as_deref().unwrap_or("").to_string();
        
        // Scope for initial state retrieval and initialization
        let (mut caret, mut anchor) = {
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            if st.history.stack.is_empty() || st.last_value != value {
                st.history.push(value.clone(), st.caret, st.caret);
                st.last_value = value.clone();
            }
            (st.caret, st.anchor)
        };

        caret = Self::clamp_caret_to_value(&value, caret);
        anchor = Self::clamp_caret_to_value(&value, anchor);
        
        let sel = if caret != anchor { Some((anchor, caret)) } else { None };

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
                let (s, e) = if caret <= anchor { (caret, anchor) } else { (anchor, caret) };
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
                        let (ctrl_or_super, shift) = (((modifiers & 4) != 0) || ((modifiers & 8) != 0), (modifiers & 1) != 0);
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
                let (txt, nc) = Self::insert_text(&value, caret, sel, &c.to_string());
                next_text = Some(txt);
                next_caret = nc;
                next_anchor = nc;
                handled = true;
            }
            KeyCode::Backspace => {
                let (txt, nc) = if (modifiers & 2) != 0 && sel.is_none() { // Ctrl+Backspace
                    let mut at = caret;
                    while at > 0 {
                        let prev = Self::prev_grapheme_boundary(&value, at);
                        let ch = value[prev..].chars().next().unwrap_or('\0');
                        if !ch.is_whitespace() { at = prev; break; }
                        at = prev;
                    }
                    while at > 0 {
                        let prev = Self::prev_grapheme_boundary(&value, at);
                        let ch = value[prev..].chars().next().unwrap_or('\0');
                        if ch.is_alphanumeric() || ch == '_' { at = prev; } else { break; }
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
                let prev = if (modifiers & 2) != 0 { // Ctrl+Left
                    Self::prev_word_boundary(&value, caret)
                } else {
                    Self::prev_grapheme_boundary(&value, caret)
                };
                next_caret = prev;
                if (modifiers & 1) != 0 { next_anchor = anchor; } else { next_anchor = prev; }
                handled = true;
            }
            KeyCode::Right => {
                let next = if (modifiers & 2) != 0 { // Ctrl+Right
                    Self::next_word_boundary(&value, caret)
                } else {
                    Self::next_grapheme_boundary(&value, caret)
                };
                next_caret = next;
                if (modifiers & 1) != 0 { next_anchor = anchor; } else { next_anchor = next; }
                handled = true;
            }
            KeyCode::Home => {
                next_caret = 0;
                if (modifiers & 1) != 0 { next_anchor = anchor; } else { next_anchor = 0; }
                handled = true;
            }
            KeyCode::End => {
                let end = value.len();
                next_caret = end;
                if (modifiers & 1) != 0 { next_anchor = anchor; } else { next_anchor = end; }
                handled = true;
            }
            KeyCode::Enter => {
                if semantics.multiline {
                    let (txt, nc) = Self::insert_text(&value, caret, sel, "\n");
                    next_text = Some(txt);
                    next_caret = nc;
                    next_anchor = nc;
                    handled = true;
                }
            }
            KeyCode::Up => {
                if semantics.multiline {
                    self.handle_vertical_navigation(ctx, focused_id, semantics, &value, caret, modifiers, true);
                    return true; // Return early as handle_vertical_navigation does its own state update
                }
            }
            KeyCode::Down => {
                if semantics.multiline {
                    self.handle_vertical_navigation(ctx, focused_id, semantics, &value, caret, modifiers, false);
                    return true;
                }
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
        } else if handled {
            // Cursor movement only
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            st.caret = next_caret;
            st.anchor = next_anchor;
            Self::auto_scroll_textinput(ctx, focused_id);
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
                                let value = semantics.value.as_deref().unwrap_or("").to_string();
                                let st = ctx.text_edit.get_mut_or_default(focused_id);
                                let caret = Self::clamp_caret_to_value(&value, st.caret);
                                let sel = if st.caret != st.anchor { Some((st.anchor, st.caret)) } else { None };
                                let (new_text, new_caret) = Self::insert_text(&value, caret, sel, text);
                                self.dispatch_change(ctx, semantics, focused_id, new_text, new_caret);
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

    fn dispatch_change(&self, ctx: &mut ControllerContext, semantics: &fission_ir::Semantics, node_id: NodeId, new_text: String, new_caret: usize) {
        if let Some(action_entry) = semantics.actions.entries.first() {
            let payload = serde_json::to_vec(&new_text).unwrap();
            let envelope = ActionEnvelope {
                id: ActionId::from_u128(action_entry.action_id),
                payload,
            };
            ctx.dispatched_actions.push((node_id, envelope));
            
            // State update moved to handle_key to avoid double borrow
            
            Self::auto_scroll_textinput(ctx, node_id);
        }
    }

    fn clamp_caret_to_value(value: &str, caret: usize) -> usize {
        if caret > value.len() { value.len() } else { caret }
    }

    fn prev_grapheme_boundary(value: &str, idx: usize) -> usize {
        let mut last = 0;
        for (pos, _) in value.grapheme_indices(true) {
            if pos >= idx { break; }
            last = pos;
        }
        last
    }

    fn next_grapheme_boundary(value: &str, idx: usize) -> usize {
        for (pos, _) in value.grapheme_indices(true) {
            if pos > idx { return pos; }
        }
        value.len()
    }

    fn prev_word_boundary(value: &str, idx: usize) -> usize {
        let mut at = idx.min(value.len());
        while at > 0 {
            let prev = Self::prev_grapheme_boundary(value, at);
            let ch = value[prev..].chars().next().unwrap_or('\0');
            if !ch.is_whitespace() { at = prev; break; }
            at = prev;
        }
        while at > 0 {
            let prev = Self::prev_grapheme_boundary(value, at);
            let ch = value[prev..].chars().next().unwrap_or('\0');
            if ch.is_alphanumeric() || ch == '_' { at = prev; } else { break; }
        }
        at
    }

    fn next_word_boundary(value: &str, idx: usize) -> usize {
        let mut at = idx.min(value.len());
        while at < value.len() {
            let next = Self::next_grapheme_boundary(value, at);
            let ch = value[at..].chars().next().unwrap_or('\0');
            if !ch.is_whitespace() { at = next; break; }
            at = next;
        }
        while at < value.len() {
            let next = Self::next_grapheme_boundary(value, at);
            let ch = value[at..].chars().next().unwrap_or('\0');
            if ch.is_alphanumeric() || ch == '_' { at = next; } else { break; }
            at = next;
        }
        at
    }

    fn delete_prev_grapheme(value: &str, caret: usize, sel: Option<(usize,usize)>) -> (String, usize) {
        if let Some((a,b)) = sel {
            let (s,e) = if a<=b {(a,b)} else {(b,a)};
            let mut out = String::with_capacity(value.len() - (e-s));
            out.push_str(&value[..s]);
            out.push_str(&value[e..]);
            return (out, s);
        }
        let at = caret.min(value.len());
        if at == 0 { return (value.to_string(), 0); }
        let prev = Self::prev_grapheme_boundary(value, at);
        let mut out = String::with_capacity(value.len() - (at-prev));
        out.push_str(&value[..prev]);
        out.push_str(&value[at..]);
        (out, prev)
    }

    fn insert_text(value: &str, caret: usize, sel: Option<(usize,usize)>, text: &str) -> (String, usize) {
        let (s,e) = sel.map(|(a,b)| if a<=b {(a,b)} else {(b,a)}).unwrap_or((caret, caret));
        let mut out = String::with_capacity(value.len() - (e-s) + text.len());
        out.push_str(&value[..s]);
        out.push_str(text);
        out.push_str(&value[e..]);
        (out, s + text.len())
    }

    fn find_scroll_container_and_text_op(ir: &fission_ir::CoreIR, root: NodeId, multiline_semantics: bool) -> Option<(NodeId, NodeId, op::FlexDirection)> {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if let Some(n) = ir.nodes.get(&id) {
                if let Op::Layout(op::LayoutOp::Scroll { direction, .. }) = &n.op {
                    let matches_multiline_config = (multiline_semantics && *direction == op::FlexDirection::Column) ||
                                                   (!multiline_semantics && *direction == op::FlexDirection::Row);
                    if matches_multiline_config {
                        let mut q = vec![id]; // Start BFS from scroll node to find text
                        while let Some(cid) = q.pop() {
                            if let Some(cn) = ir.nodes.get(&cid) {
                                if let Op::Paint(fission_ir::PaintOp::DrawText { .. }) = cn.op {
                                    return Some((id, cid, *direction));
                                }
                                for &gc in &cn.children { q.push(gc); }
                            }
                        }
                        return None; // Should find text inside. For now, assume it's directly related. 
                    }
                }
                for &c in &n.children { stack.push(c); }
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
                        if has_paint { return Some(id); }
                    }
                }
                for &c in &n.children { q.push(c); }
            }
        }
        None
    }

    fn caret_from_point_in_text_fallback(
        value: &str, 
        font_size: f32, 
        viewport_x: f32, viewport_w: f32, content_w: f32, scroll_offset: f32, point_x: f32
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
                if let Op::Semantics(sem) = &node.op { sem.multiline } else { false }
            } else { false };

            if let Some((scroll_id, _text_op_node_id, scroll_direction)) = Self::find_scroll_container_and_text_op(ctx.ir, text_root, is_multiline) {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_origin = scroll_geom.rect.origin;
                    let viewport_size = scroll_geom.rect.size;
                    
                    let current_text_value = if let Some(node) = ctx.ir.nodes.get(&text_root) {
                        if let Op::Semantics(sem) = &node.op { sem.value.clone().unwrap_or_default() } else { String::new() }
                    } else { String::new() };

                    let current_caret_idx = if let Some(st) = ctx.text_edit.get(text_root) { st.caret } else { 0 };

                    let (caret_x, caret_y) = measurer.get_caret_position(
                        &current_text_value, font_size, Some(viewport_size.width), current_caret_idx
                    );

                    let mut offset = ctx.scroll.get_offset(scroll_id);
                    
                    if scroll_direction == op::FlexDirection::Row {
                        // Handle horizontal scrolling for single-line text
                        let caret_left = caret_x;
                        let caret_width = 2.0f32;
                        let caret_right = caret_left + caret_width;

                        let margin_left = 2.0f32;
                        let margin_right = 3.0f32; 

                        let visible_left = (caret_left - offset) - viewport_origin.x;
                        let visible_right = (caret_right - offset) - viewport_origin.x;

                        if visible_right > (viewport_size.width - margin_right) {
                            offset = (caret_right - (viewport_origin.x + viewport_size.width - margin_right)).max(0.0f32);
                        } else if visible_left < margin_left {
                            offset = (caret_left - (viewport_origin.x + margin_left)).max(0.0f32);
                        }
                        let content_w = scroll_geom.content_size.width.max(viewport_size.width);
                        let max_offset = (content_w - viewport_size.width).max(0.0f32);
                        offset = offset.clamp(0.0f32, max_offset);
                        ctx.scroll.set_offset(scroll_id, offset);
                    } else { // op::FlexDirection::Column
                        // Handle vertical scrolling for multi-line text
                        let caret_top = caret_y;
                        let caret_height = measurer.measure("Tg", font_size, Some(viewport_size.width)).1;
                        let caret_bottom = caret_top + caret_height;

                        let margin_top = 2.0f32;
                        let margin_bottom = 3.0f32;
                        
                        let visible_top = (caret_top - offset) - viewport_origin.y;
                        let visible_bottom = (caret_bottom - offset) - viewport_origin.y;

                        if visible_bottom > (viewport_size.height - margin_bottom) {
                            offset = (caret_bottom - (viewport_origin.y + viewport_size.height - margin_bottom)).max(0.0f32);
                        } else if visible_top < margin_top {
                            offset = (caret_top - (viewport_origin.y + margin_top)).max(0.0f32);
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

    fn handle_vertical_navigation(&mut self, ctx: &mut ControllerContext, focused_id: NodeId, semantics: &Semantics, value: &str, caret: usize, modifiers: u8, is_up: bool) {
        if let Some(measurer) = ctx.measurer {
            if let Some((scroll_id, _text_op_node_id, scroll_direction)) = Self::find_scroll_container_and_text_op(ctx.ir, focused_id, semantics.multiline) {
                if let Some(scroll_geom) = ctx.layout.get_node_geometry(scroll_id) {
                    let viewport_w = scroll_geom.rect.size.width;
                    let font_size = 16.0;

                    let (current_caret_x, current_caret_y) = measurer.get_caret_position(
                        value, font_size, Some(viewport_w), caret
                    );

                    let line_metrics = measurer.get_line_metrics(value, font_size, Some(viewport_w));
                    
                    let mut current_line_idx = 0;
                    for (idx, line) in line_metrics.iter().enumerate() {
                        if caret >= line.start_index && caret <= line.end_index {
                            current_line_idx = idx;
                            break;
                        }
                    }

                    let target_line_idx = if is_up {
                        current_line_idx.saturating_sub(1)
                    } else {
                        (current_line_idx + 1).min(line_metrics.len().saturating_sub(1))
                    };

                    if let Some(target_line) = line_metrics.get(target_line_idx) {
                        // Calculate target y (relative to text box) by placing caret at same X as before.
                        // We need the Y coordinate of the target line (e.g. baseline or mid-height).
                        // The Y passed to hit_test should be relative to the *start of the text block*,
                        // which is the current origin of the scroll_geom.rect.
                        let target_y = target_line.baseline;
                        
                        let new_caret_pos = measurer.hit_test(
                            value,
                            font_size,
                            Some(viewport_w),
                            current_caret_x,
                            target_y,
                        );

                        let st = ctx.text_edit.get_mut_or_default(focused_id);
                        st.caret = new_caret_pos;
                        if (modifiers & 1) == 0 { st.anchor = new_caret_pos; } // If no shift, collapse selection
                        Self::auto_scroll_textinput(ctx, focused_id);
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
    viewport_x: f32, viewport_w: f32, content_w: f32, scroll_offset: f32, point_x: f32
) -> usize {
    let local_x = (point_x - viewport_x) + scroll_offset;
    if local_x <= 0.0 { return 0; }
    let max_x = content_w.max(viewport_w);
    if local_x >= max_x { return value.len(); }

    if let Some(measurer) = measurer {
        // This function is for single line mostly. measurer.hit_test is better.
        // Just re-use hit_test with dummy Y
        measurer.hit_test(value, font_size, Some(viewport_w), local_x, 0.0)
    } else {
        TextInputController::caret_from_point_in_text_fallback(
            value, font_size, viewport_x, viewport_w, content_w, scroll_offset, point_x
        )
    }
}
