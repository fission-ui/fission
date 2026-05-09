use super::{ControllerContext, InputController};
use crate::event::{
    InputEvent, KeyCode, KeyEvent, PointerEvent, MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_SUPER,
};
use crate::ActionEnvelope;
use crate::ActionId;
use fission_ir::FlexDirection;
use fission_ir::{
    op::{self, LayoutOp, Op},
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
                modifiers,
                ..
            }) => {
                // Only keep handling pointer-down inside the already-focused input
                // if the hit test still resolves into that subtree. Otherwise we
                // must fall through so Runtime can move focus to a different
                // widget instead of swallowing the click.
                let effective_focused = if let Some(focused_id) = ctx.interaction.focused {
                    let hit = crate::hit_test::hit_test_with_scroll(
                        ctx.ir, ctx.layout, ctx.scroll, *point,
                    );
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
                        None
                    }
                } else {
                    // If nothing is focused, try to find the TextInput under the
                    // click point and focus + place the caret in one step.
                    let hit = crate::hit_test::hit_test_with_scroll(
                        ctx.ir, ctx.layout, ctx.scroll, *point,
                    );
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
                                            let st =
                                                ctx.text_edit.get_mut_or_default(focused_id);
                                            st.caret = caret;
                                            if !Self::has_shift(*modifiers) {
                                                st.anchor = caret;
                                            }
                                            st.anchor
                                        };
                                        Self::dispatch_cursor_change(
                                            ctx, sem, focused_id, caret, anchor,
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
                                                    ctx,
                                                    focused_id,
                                                    value,
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
                            let (line_start, _) =
                                Self::current_line_bounds(ctx, focused_id, semantics, &value, caret);
                            next_caret = line_start;
                            next_anchor = if shift { anchor } else { line_start };
                            handled = true;
                        }
                        'e' => {
                            let (_, line_end) =
                                Self::current_line_bounds(ctx, focused_id, semantics, &value, caret);
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
                        if let Some(inserted) = Self::prepare_inserted_text(
                            semantics,
                            &value,
                            s,
                            e,
                            &ch.to_string(),
                        ) {
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
                next_caret = if semantics.multiline
                    && !(Self::has_ctrl(modifiers) && !is_apple)
                {
                    Self::current_line_bounds(ctx, focused_id, semantics, &value, caret).0
                } else {
                    0
                };
                next_anchor = if shift { anchor } else { next_caret };
                handled = true;
            }
            KeyCode::End => {
                next_caret = if semantics.multiline
                    && !(Self::has_ctrl(modifiers) && !is_apple)
                {
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
                    self.handle_page_navigation(ctx, focused_id, semantics, &value, caret, modifiers, true);
                    return true;
                }
            }
            KeyCode::PageDown => {
                if semantics.multiline {
                    self.handle_page_navigation(ctx, focused_id, semantics, &value, caret, modifiers, false);
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
            return true;
        }

        if let Some((range, replacement)) = next_edit {
            // Apply text change
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            let txt = st.apply_edit(range, &replacement, next_caret, next_anchor);
            self.dispatch_change(ctx, semantics, focused_id, txt);
            Self::dispatch_cursor_change(ctx, semantics, focused_id, next_caret, next_anchor);
        } else if handled {
            // Cursor movement only
            let st = ctx.text_edit.get_mut_or_default(focused_id);
            st.caret = next_caret;
            st.anchor = next_anchor;
            st.clear_preedit();
            Self::auto_scroll_textinput(ctx, focused_id);
            Self::dispatch_cursor_change(ctx, semantics, focused_id, next_caret, next_anchor);
        }

        handled
    }

    fn is_apple_platform() -> bool {
        cfg!(target_os = "macos") || cfg!(target_os = "ios")
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
                        let line_metrics =
                            measurer.get_line_metrics(value, font_size, Some(scroll_geom.rect.size.width));
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

    fn apply_text_capitalization(
        mode: TextCapitalization,
        prefix: &str,
        inserted: &str,
    ) -> String {
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

        let mut inserted = Self::apply_input_type_filter(
            semantics.text_input_type,
            raw_text,
            semantics.multiline,
        );
        inserted = Self::apply_text_capitalization(
            semantics.text_capitalization,
            &current_value[..replace_start],
            &inserted,
        );
        inserted = Self::apply_formatters(&inserted, &semantics.input_formatters, semantics.multiline);

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
                let available = max_length.saturating_sub(current_chars.saturating_sub(replaced_chars));
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
                                    let new_text = st.apply_edit(start..end, &filtered_text, new_caret, new_caret);
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
            if let Some(action_entry) = semantics.actions.entries.iter().find(|e| e.trigger == trigger)
            {
                let payload = action_entry
                    .payload_data
                    .clone()
                    .unwrap_or_else(|| serde_json::to_vec(&current_value.to_string()).unwrap());
                let envelope = ActionEnvelope {
                    id: ActionId::from_u128(action_entry.action_id),
                    payload,
                };
                ctx.dispatched_actions
                    .push((node_id, envelope, crate::ActionInput::None));
                dispatched = true;
            }
        }
        dispatched
    }

    fn resolve_editing_value(
        ctx: &mut ControllerContext,
        focused_id: NodeId,
        semantic_value: &str,
    ) -> (String, usize, usize) {
        let st = ctx.text_edit.get_mut_or_default(focused_id);
        st.sync_from_model(semantic_value);
        let value = st.committed_text();
        (value, st.caret, st.anchor)
    }

    fn display_value_for_metrics(
        ctx: &mut ControllerContext,
        focused_id: NodeId,
        semantic_value: &str,
    ) -> String {
        let state = ctx.text_edit.get_mut_or_default(focused_id);
        state.sync_from_model(semantic_value);
        state.display_text().0
    }

    fn mask_text_for_metrics(text: &str) -> String {
        let mut masked = String::new();
        for _ in text.graphemes(true) {
            masked.push('•');
        }
        masked
    }

    fn masked_byte_offset_from_source(source: &str, masked: &str, source_byte_offset: usize) -> usize {
        let clamped = source_byte_offset.min(source.len());
        let grapheme_count = source[..clamped].graphemes(true).count();
        masked
            .grapheme_indices(true)
            .nth(grapheme_count)
            .map(|(idx, _)| idx)
            .unwrap_or(masked.len())
    }

    fn source_byte_offset_from_masked(source: &str, masked: &str, masked_byte_offset: usize) -> usize {
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
        // The renderer calls `layout_rich(…, if bounds.width() > 0 { Some(w) } else { None })`.
        // The bounds rect for a text node equals the layout geometry rect whose width comes
        // from the scroll container (or the text node itself).  We replicate this here so
        // the cache key matches and we reuse the SAME Parley layout the renderer painted.
        let render_width = if scroll_geom.rect.size.width > 0.0 {
            Some(scroll_geom.rect.size.width)
        } else {
            None
        };

        if !prefer_plain_text {
            if let Some(runs) = Self::extract_rich_runs(ir, focused_id) {
                return measurer.hit_test_rich(&runs, render_width, local_x, local_y);
            }
        }
        let font_size = Self::extract_font_size(ir, focused_id).unwrap_or(13.0);
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

                    let (current_text_value, metric_text, masked) =
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
                                (display_value, metric_text, sem.masked)
                            } else {
                                (String::new(), String::new(), false)
                            }
                        } else {
                            (String::new(), String::new(), false)
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
                        let target_end =
                            Self::trim_line_end(value, target_line.end_index.max(target_line.start_index));
                        new_caret_pos =
                            new_caret_pos.clamp(target_line.start_index, target_end.max(target_line.start_index));

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
