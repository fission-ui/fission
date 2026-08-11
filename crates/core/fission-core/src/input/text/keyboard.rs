use super::*;

impl TextInputController {
    pub(super) fn handle_key(
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
}
