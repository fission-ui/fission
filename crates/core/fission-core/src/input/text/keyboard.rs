use super::*;

impl TextInputController {
    pub(super) fn handle_editing_command(
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
                        let next = ctx.text_edit.get_mut_or_default(focused_id).apply_edit(
                            start..end,
                            "",
                            start,
                            start,
                        );
                        self.dispatch_change(ctx, &semantics, focused_id, next);
                        Self::dispatch_cursor_change(ctx, &semantics, focused_id, start, start);
                    }
                }
            }
            EditingCommand::Paste(text) => {
                if !semantics.read_only && !text.is_empty() {
                    let (start, end) = selection.unwrap_or((caret, caret));
                    if let Some(inserted) =
                        Self::prepare_inserted_text(&semantics, &value, start, end, text)
                    {
                        let next_caret = start + inserted.len();
                        let next = ctx.text_edit.get_mut_or_default(focused_id).apply_edit(
                            start..end,
                            &inserted,
                            next_caret,
                            next_caret,
                        );
                        self.dispatch_change(ctx, &semantics, focused_id, next);
                        Self::dispatch_cursor_change(
                            ctx, &semantics, focused_id, next_caret, next_caret,
                        );
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
