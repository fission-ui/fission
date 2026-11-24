use super::*;

impl TextInputController {
    pub(super) fn handle_ime(
        &mut self,
        ctx: &mut ControllerContext,
        ime: &crate::event::ImeEvent,
    ) -> bool {
        match ime {
            crate::event::ImeEvent::Commit { text } => {
                if ctx
                    .interaction
                    .focused
                    .is_some_and(|focused_id| Self::text_input_semantics(ctx, focused_id).is_none())
                    && Self::dispatch_text_entry(
                        ctx,
                        crate::TextEntryInput {
                            text: text.clone(),
                            source: crate::TextEditSource::Ime,
                            phase: crate::TextEntryPhase::CompositionCommitted,
                            cursor: None,
                        },
                    )
                {
                    return true;
                }
                if let Some(focused_id) = ctx.interaction.focused {
                    if let Some(node) = ctx.ir.nodes.get(&focused_id) {
                        if let Op::Semantics(semantics) = &node.op {
                            if semantics.supports_text_editing() {
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
                    if Self::dispatch_text_entry(
                        ctx,
                        crate::TextEntryInput {
                            text: text.clone(),
                            source: crate::TextEditSource::Ime,
                            phase: crate::TextEntryPhase::CompositionPreedit,
                            cursor: *cursor,
                        },
                    ) {
                        return true;
                    }
                    if crate::hit_test::is_interaction_inert(ctx.ir, focused_id) {
                        return true;
                    }
                    if Self::text_input_semantics(ctx, focused_id).is_none() {
                        return false;
                    }
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
                    ctx.interaction.clear_active_descendant(focused_id);
                    let st = ctx.text_edit.get_mut_or_default(focused_id);
                    st.set_preedit(text.clone(), *cursor);
                    Self::auto_scroll_textinput(ctx, focused_id);
                    return true;
                }
            }
            crate::event::ImeEvent::Cancel => {
                if let Some(focused_id) = ctx.interaction.focused {
                    if Self::dispatch_text_entry(
                        ctx,
                        crate::TextEntryInput {
                            text: String::new(),
                            source: crate::TextEditSource::Ime,
                            phase: crate::TextEntryPhase::CompositionCancelled,
                            cursor: None,
                        },
                    ) {
                        return true;
                    }
                    if crate::hit_test::is_interaction_inert(ctx.ir, focused_id) {
                        return true;
                    }
                    if Self::text_input_semantics(ctx, focused_id).is_none() {
                        return false;
                    }
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
}
