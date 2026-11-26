use super::*;

impl Lower for TextInput {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let input_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let is_focused = cx.runtime_state.interaction.is_focused(input_id);
        let is_hovered = cx.runtime_state.interaction.is_hovered(input_id);

        // Resolve the live controlled/editing value before visual state so the
        // painted error state and exposed validation state cannot disagree.
        let model_text = self
            .editing_value
            .as_ref()
            .map(|value| value.text.as_str())
            .unwrap_or(self.value.as_str());
        let controlled_selection = self.editing_value.as_ref().map(|value| {
            (
                value.selection.extent.utf8_offset(),
                value.selection.base.utf8_offset(),
            )
        });
        let session = cx.runtime_state.text_edit.get(input_id);
        let pending_model_transform = session.is_some_and(|state| {
            state.pending_model_sync
                && state.preedit.is_none()
                && state.committed_text() != model_text
                && state.last_model_text != model_text
        });
        let retained_session = if is_focused {
            session.filter(|state| {
                (state.pending_model_sync
                    && (state.committed_text() == model_text
                        || state.last_model_text == model_text))
                    || state.preedit.is_some()
                    || (self.restoration_id.is_some() && model_text.is_empty())
                    || state.committed_text() == model_text
            })
        } else {
            None
        };
        let session_display = retained_session.map(|state| state.display_text());
        // A collapsed caret cannot be carried across a model transform: the
        // offset was measured against the local edit, and the model may have
        // inserted or removed text anywhere before it. Move it to the end of
        // the value actually being rendered.
        //
        // This applies whether or not the session was retained. A transform is
        // precisely the case where it is not retained, so checking it only on
        // the retained path left the rule unreachable.
        let selection_for = |state: &crate::env::TextEditState| {
            if pending_model_transform && state.caret == state.anchor {
                (model_text.len(), model_text.len())
            } else {
                (
                    clamp_text_offset(model_text, state.caret),
                    clamp_text_offset(model_text, state.anchor),
                )
            }
        };
        let model_selection = retained_session
            .map(selection_for)
            .or(controlled_selection)
            .or_else(|| session.map(selection_for))
            .unwrap_or((model_text.len(), model_text.len()));
        let semantic_value = retained_session
            .map(|state| state.committed_text())
            .unwrap_or_else(|| model_text.to_string());
        let live_value = self
            .editing_value
            .clone()
            .unwrap_or_else(|| crate::TextEditingValue::from_text(semantic_value.clone()));
        let grapheme_len = live_value.text.graphemes(true).count();
        let pattern_invalid = self.validation_pattern.as_deref().is_some_and(|pattern| {
            let anchored = format!("^(?:{pattern})$");
            match regex_lite::Regex::new(&anchored) {
                Ok(regex) => !regex.is_match(&live_value.text),
                Err(_) => {
                    #[cfg(debug_assertions)]
                    report_invalid_validation_pattern();
                    false
                }
            }
        });
        let semantics_required = self
            .semantics
            .as_ref()
            .is_some_and(|semantics| semantics.required);
        let form_field_context = cx.form_field_context();
        let effective_required = self.required
            || semantics_required
            || form_field_context.is_some_and(|context| context.required);
        let declared_validation_state =
            if form_field_context.is_some_and(|context| context.invalid_message.is_some()) {
                TextFieldValidationState::Invalid
            } else if self.validation_state == TextFieldValidationState::Unvalidated {
                self.semantics
                    .as_ref()
                    .map(|semantics| semantics.validation_state)
                    .unwrap_or(TextFieldValidationState::Unvalidated)
            } else {
                self.validation_state
            };
        let missing_required = effective_required && live_value.text.is_empty();
        let constraint_invalid = missing_required
            || self
                .min_length
                .is_some_and(|minimum| grapheme_len < minimum)
            || self
                .max_length
                .is_some_and(|maximum| grapheme_len > maximum)
            || pattern_invalid;
        let custom_validation = self
            .validator
            .as_ref()
            .map(|validator| validator.validate(&live_value));
        let resolved_validation_state = custom_validation
            .as_ref()
            .map(|result| result.state)
            .unwrap_or(if constraint_invalid {
                TextFieldValidationState::Invalid
            } else {
                declared_validation_state
            });
        let effective_validation_state = if self.error_text.is_some() {
            TextFieldValidationState::Invalid
        } else {
            resolved_validation_state
        };
        let effective_validation_message = custom_validation
            .and_then(|result| result.message)
            .or_else(|| self.validation_message.clone())
            .or_else(|| {
                self.semantics
                    .as_ref()
                    .and_then(|semantics| semantics.validation_message.clone())
            })
            .or_else(|| form_field_context.and_then(|context| context.invalid_message.clone()));
        let is_invalid = effective_validation_state == TextFieldValidationState::Invalid;
        // An empty required field still fails form validation, but it is only drawn as an error
        // once the user has visited and left it, so a new form does not open full of red fields.
        let visited = session.is_some() && !is_focused;
        let only_missing_required = missing_required
            && self.error_text.is_none()
            && effective_validation_message.is_none()
            && declared_validation_state != TextFieldValidationState::Invalid
            && !self
                .min_length
                .is_some_and(|minimum| grapheme_len < minimum)
            && !pattern_invalid;
        let show_invalid = is_invalid && (visited || !only_missing_required);

        let theme = &cx.env.theme.components.text_input;
        let tokens = &cx.env.theme.tokens;
        let component_state = if !self.enabled {
            ComponentState::Disabled
        } else if show_invalid {
            ComponentState::Error
        } else if is_focused {
            ComponentState::Focus
        } else if is_hovered {
            ComponentState::Hover
        } else {
            ComponentState::Default
        };
        let component_style = theme.resolve(self.size, component_state);

        let text_scale = self.text_scale.unwrap_or(1.0).max(0.0);
        let uses_narrow_typography = self.font_size.is_none()
            && cx.env.viewport_size.width.is_finite()
            && cx.env.viewport_size.width > 0.0
            && cx.env.viewport_size.width < 768.0;
        let font_size = self.font_size.unwrap_or(if uses_narrow_typography {
            16.0
        } else {
            component_style.font_size.unwrap_or(theme.font_size)
        }) * text_scale;
        let text_color = self
            .text_color
            .unwrap_or(component_style.text_color.unwrap_or(theme.text_color));
        let selection_color = self
            .selection_color
            .unwrap_or(tokens.colors.primary.with_alpha(52));
        let selection_text_color = self.selection_text_color.unwrap_or(text_color);
        let placeholder_color = self.placeholder_color.unwrap_or(
            theme
                .placeholder_style
                .text_color
                .unwrap_or(theme.placeholder_color),
        );
        let cursor_color = self.cursor_color.unwrap_or(theme.focus_color);
        let cursor_width = self.cursor_width.unwrap_or(2.0);
        let font_weight = self
            .font_weight
            .unwrap_or(component_style.font_weight.unwrap_or(theme.font_weight));
        let line_height = self
            .line_height
            .or(component_style.line_height)
            .map(|value| value * text_scale);
        let letter_spacing = self
            .letter_spacing
            .or(component_style.letter_spacing)
            .unwrap_or(0.0)
            * text_scale;
        let recipe_border = component_style.border.as_ref();
        let border_fill_override = if is_focused {
            self.focus_border_color
        } else {
            self.border_color
        };
        let border_fill = border_fill_override
            .map(Fill::Solid)
            .or_else(|| recipe_border.map(|border| border.fill.clone()))
            .unwrap_or_else(|| {
                Fill::Solid(if is_focused {
                    theme.focus_color
                } else {
                    theme.border_color
                })
            });
        let border_width = if is_focused {
            self.focus_border_width.unwrap_or(
                recipe_border
                    .map(|border| border.width)
                    .unwrap_or(theme.border_width),
            )
        } else {
            self.border_width.unwrap_or(
                recipe_border
                    .map(|border| border.width)
                    .unwrap_or(theme.border_width),
            )
        };
        let border_radius = self
            .border_radius
            .unwrap_or(component_style.radius.unwrap_or(theme.radius));
        let recipe_padding = self.padding.unwrap_or(component_style.padding_box(
            component_style.padding_x.unwrap_or(theme.padding_h),
            component_style.padding_y.unwrap_or(4.0),
        ));
        let layout_border_width = if self.borderless {
            0.0
        } else {
            [
                ComponentState::Default,
                ComponentState::Hover,
                ComponentState::Focus,
                ComponentState::Error,
                ComponentState::Disabled,
            ]
            .into_iter()
            .filter_map(|state| {
                theme
                    .resolve(self.size, state)
                    .border
                    .map(|border| border.width)
            })
            .chain(self.border_width)
            .chain(self.focus_border_width)
            .filter(|width| width.is_finite())
            .fold(0.0_f32, f32::max)
            .max(0.0)
        };
        let content_padding = [
            recipe_padding[0] + layout_border_width,
            recipe_padding[1] + layout_border_width,
            recipe_padding[2] + layout_border_width,
            recipe_padding[3] + layout_border_width,
        ];
        let base_text_style = fission_ir::op::TextStyle {
            font_size,
            color: text_color,
            underline: false,
            font_family: self
                .font_family
                .clone()
                .or_else(|| Some(cx.env.theme.tokens.typography.font_family_sans.clone())),
            locale: self.locale.clone(),
            font_weight,
            font_style: self.font_style.into(),
            line_height,
            letter_spacing,
            background_color: None,
            typography: Default::default(),
        };

        let resolved_label = self
            .label
            .as_ref()
            .map(|label| Self::resolve_text_content(label, cx));
        let resolved_placeholder = self
            .placeholder
            .as_ref()
            .map(|placeholder| Self::resolve_text_content(placeholder, cx));

        // 1. Background (skipped in borderless mode). Shadows are separate
        // paint nodes so every layer is preserved without changing the field's
        // layout or conflating a focus ring with its border. Outer shadows are
        // behind the field; inset shadows are above its background.
        let background_ids = if self.borderless {
            Vec::new()
        } else {
            let mut ids = Vec::new();
            for shadow in component_style
                .shadows
                .iter()
                .filter(|shadow| !shadow.inset)
            {
                ids.push(
                    IrBuilder::new(
                        cx.next_node_id(),
                        Op::Paint(PaintOp::DrawRect {
                            fill: None,
                            stroke: None,
                            corner_radius: border_radius,
                            shadow: Some(shadow.to_box_shadow()),
                            corner_radii: None,
                            border_sides: None,
                        }),
                    )
                    .build(cx),
                );
            }
            ids.push(
                IrBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: Some(
                            self.background_fill
                                .clone()
                                .or_else(|| component_style.background.clone())
                                .unwrap_or(Fill::Solid(tokens.colors.background)),
                        ),
                        stroke: Some(Stroke {
                            fill: border_fill,
                            width: border_width,
                            dash_array: component_style.border_dash.clone(),
                            dash_offset: 0.0,
                            trim: None,
                            line_cap: fission_ir::op::LineCap::Butt,
                            line_join: fission_ir::op::LineJoin::Miter,
                        }),
                        corner_radius: border_radius,
                        shadow: None,
                        corner_radii: None,
                        border_sides: None,
                    }),
                )
                .build(cx),
            );
            for shadow in component_style.shadows.iter().filter(|shadow| shadow.inset) {
                ids.push(
                    IrBuilder::new(
                        cx.next_node_id(),
                        Op::Paint(PaintOp::DrawRect {
                            fill: None,
                            stroke: None,
                            corner_radius: border_radius,
                            shadow: Some(shadow.to_box_shadow()),
                            corner_radii: None,
                            border_sides: None,
                        }),
                    )
                    .build(cx),
                );
            }
            ids
        };

        // 2. Text Preparation
        let displayed_preedit_range = session_display.as_ref().and_then(|(_, range)| *range);
        let (display_text, preedit_range, preedit_cursor_range, caret, anchor) =
            if self.obscure_text {
                let mut combined = model_text.to_string();
                if let Some((display, _)) = &session_display {
                    combined = display.clone();
                }
                let (caret, anchor) = retained_session
                    .map(|state| (state.caret, state.anchor))
                    .unwrap_or(model_selection);
                let masked = Self::mask_text(&combined, self.obscuring_character);
                let mapped_caret = Self::masked_byte_offset(&combined, &masked, caret);
                let mapped_anchor = Self::masked_byte_offset(&combined, &masked, anchor);
                (masked, None, None, mapped_caret, mapped_anchor)
            } else {
                match session_display.as_ref() {
                    Some((combined, preedit_range)) => {
                        let (caret, anchor) = retained_session
                            .map(|state| (state.caret, state.anchor))
                            .unwrap_or(model_selection);
                        let cursor_range =
                            retained_session.and_then(|state| state.display_preedit_cursor_range());
                        let preedit_range = (*preedit_range).or_else(|| {
                            retained_session.and_then(|state| {
                                state.platform_composing.map(|range| {
                                    (range.start.utf8_offset(), range.end.utf8_offset())
                                })
                            })
                        });
                        (combined.clone(), preedit_range, cursor_range, caret, anchor)
                    }
                    None => (
                        model_text.to_string(),
                        self.editing_value
                            .as_ref()
                            .and_then(|value| value.composing)
                            .map(|range| (range.start.utf8_offset(), range.end.utf8_offset())),
                        None,
                        model_selection.0,
                        model_selection.1,
                    ),
                }
            };
        let display_breaks_for_paint = if self.obscure_text {
            Vec::new()
        } else {
            retained_session
                .and_then(|state| {
                    let committed_text = state.committed_text();
                    if committed_text != model_text {
                        // The model has not caught up with a local edit, so the
                        // caller's byte offsets no longer identify the displayed
                        // value. Hide the layout hints until they can be mapped.
                        return None;
                    }
                    let preedit = state.preedit.as_ref()?;
                    let displayed_preedit_range = displayed_preedit_range?;
                    Some(map_display_breaks_through_preedit(
                        &self.display_breaks,
                        preedit.range,
                        displayed_preedit_range.1 - displayed_preedit_range.0,
                    ))
                })
                .unwrap_or_else(|| {
                    if retained_session.is_some_and(|state| state.committed_text() != model_text) {
                        Vec::new()
                    } else {
                        self.display_breaks.clone()
                    }
                })
        };
        let mut paragraph_styles_for_paint = if self.obscure_text {
            Vec::new()
        } else {
            retained_session
                .and_then(|state| {
                    let committed_text = state.committed_text();
                    if committed_text != model_text {
                        return None;
                    }
                    let preedit = state.preedit.as_ref()?;
                    let displayed_preedit_range = displayed_preedit_range?;
                    Some(map_paragraph_styles_through_preedit(
                        &self.paragraph_styles,
                        preedit.range,
                        displayed_preedit_range.1 - displayed_preedit_range.0,
                    ))
                })
                .unwrap_or_else(|| {
                    if retained_session.is_some_and(|state| state.committed_text() != model_text) {
                        Vec::new()
                    } else {
                        self.paragraph_styles.clone()
                    }
                })
        };
        let needs_default_paragraph_style = !paragraph_styles_for_paint.is_empty()
            || self.text_align != IrTextAlign::Start
            || self.text_direction != fission_ir::op::TextDirection::Auto;
        if needs_default_paragraph_style
            && !paragraph_styles_for_paint
                .iter()
                .any(|style| style.text_offset == 0)
        {
            paragraph_styles_for_paint.insert(
                0,
                TextParagraphStyleSpan {
                    text_offset: 0,
                    style: TextParagraphLayoutStyle {
                        indent: 0.0,
                        text_align: self.text_align,
                        text_direction: self.text_direction,
                    },
                },
            );
        }
        // Construct Runs
        let mut runs = Vec::new();
        if is_focused && caret != anchor {
            let (s, e) = if caret < anchor {
                (caret, anchor)
            } else {
                (anchor, caret)
            };
            let s = s.min(display_text.len());
            let e = e.min(display_text.len());

            if s > 0 {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[..s].to_string(),
                    style: base_text_style.clone(),
                });
            }
            if s < e {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[s..e].to_string(),
                    style: fission_ir::op::TextStyle {
                        color: selection_text_color,
                        background_color: Some(selection_color),
                        ..base_text_style.clone()
                    },
                });
            }
            if e < display_text.len() {
                runs.push(fission_ir::op::TextRun {
                    text: display_text[e..].to_string(),
                    style: base_text_style.clone(),
                });
            }
        } else if let Some(styled) = &self.styled_runs {
            // Preserve syntax colouring while letting the widget-level typography
            // define the default family/weight/spacing.
            let styled_text = styled
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>();
            if styled_text == display_text {
                runs = styled
                    .iter()
                    .cloned()
                    .map(|mut run| {
                        if run.style.font_family.is_none() {
                            run.style.font_family = base_text_style.font_family.clone();
                        }
                        if run.style.font_weight == 400 {
                            run.style.font_weight = base_text_style.font_weight;
                        }
                        if run.style.font_style == fission_ir::op::FontStyle::Normal {
                            run.style.font_style = base_text_style.font_style;
                        }
                        if run.style.line_height.is_none() {
                            run.style.line_height = base_text_style.line_height;
                        }
                        if run.style.letter_spacing == 0.0 {
                            run.style.letter_spacing = base_text_style.letter_spacing;
                        }
                        run
                    })
                    .collect();
            } else {
                runs.push(fission_ir::op::TextRun {
                    text: display_text.clone(),
                    style: base_text_style.clone(),
                });
            }
        } else {
            runs.push(fission_ir::op::TextRun {
                text: display_text.clone(),
                style: base_text_style.clone(),
            });
        }

        // Apply highlight_ranges by splitting existing runs at highlight boundaries
        if !self.highlight_ranges.is_empty() && !runs.is_empty() {
            let mut final_runs = Vec::new();
            let mut run_start_byte: usize = 0;

            for run in runs {
                let run_end_byte = run_start_byte + run.text.len();
                let mut cuts = Vec::new();

                for &(hs, he, color) in &self.highlight_ranges {
                    let overlap_start = hs.max(run_start_byte);
                    let overlap_end = he.min(run_end_byte);
                    if overlap_start < overlap_end {
                        cuts.push((
                            overlap_start - run_start_byte,
                            overlap_end - run_start_byte,
                            color,
                        ));
                    }
                }

                if cuts.is_empty() {
                    final_runs.push(run);
                } else {
                    cuts.sort_by_key(|c| c.0);
                    let mut pos = 0usize;
                    for (cs, ce, bg_color) in cuts {
                        if cs > pos {
                            final_runs.push(fission_ir::op::TextRun {
                                text: run.text[pos..cs].to_string(),
                                style: run.style.clone(),
                            });
                        }
                        let mut hl_style = run.style.clone();
                        hl_style.background_color = Some(bg_color);
                        final_runs.push(fission_ir::op::TextRun {
                            text: run.text[cs..ce].to_string(),
                            style: hl_style,
                        });
                        pos = ce;
                    }
                    if pos < run.text.len() {
                        final_runs.push(fission_ir::op::TextRun {
                            text: run.text[pos..].to_string(),
                            style: run.style.clone(),
                        });
                    }
                }
                run_start_byte = run_end_byte;
            }
            runs = final_runs;
        }

        if let Some((start, end)) = preedit_range {
            // Text still being composed by an input method: underlined over a faint
            // wash of the design system's accent.
            let composing_background = cx.env.theme.tokens.colors.accent.with_alpha(48);
            runs = split_runs_for_range(&runs, start, end, |style| {
                style.underline = true;
                if style.background_color.is_none() {
                    style.background_color = Some(composing_background);
                }
            });
        }

        if display_text.is_empty() && resolved_placeholder.is_some() {
            runs = vec![fission_ir::op::TextRun {
                text: resolved_placeholder.clone().unwrap(),
                style: fission_ir::op::TextStyle {
                    color: placeholder_color,
                    ..base_text_style.clone()
                },
            }];
        }

        let caret_idx = if is_focused && self.show_cursor {
            let show = cx
                .runtime_state
                .caret_visible
                .get(&input_id)
                .copied()
                .unwrap_or(true);
            if show {
                Some(
                    preedit_range
                        .map(|(_, end)| end)
                        .unwrap_or(caret)
                        .min(display_text.len()),
                )
            } else {
                None
            }
        } else {
            None
        };

        let paragraph_overflow = if self.multiline {
            fission_ir::op::TextOverflow::Clip
        } else {
            fission_ir::op::TextOverflow::Visible
        };
        let paragraph_style = Some(TextParagraphStyle {
            text_align: self.text_align,
            max_lines: None,
            overflow: paragraph_overflow,
            text_direction: self.text_direction,
            text_width_basis: fission_ir::op::TextWidthBasis::Parent,
            strut_line_height: self.strut_line_height,
            text_height_behavior: self.text_height_behavior,
        })
        .filter(|style| {
            *style
                != TextParagraphStyle {
                    text_align: IrTextAlign::Start,
                    max_lines: None,
                    overflow: paragraph_overflow,
                    text_direction: self.text_direction,
                    text_width_basis: fission_ir::op::TextWidthBasis::Parent,
                    strut_line_height: self.strut_line_height,
                    text_height_behavior: self.text_height_behavior,
                }
        });

        let text_id = IrBuilder::new(
            text_input_text_node_id(input_id),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                display_breaks: display_breaks_for_paint,
                paragraph_styles: paragraph_styles_for_paint,
                wrap: self.multiline && self.wrap_mode != TextWrapMode::NoWrap,
                caret_index: caret_idx,
                caret_color: Some(cursor_color),
                caret_width: Some(cursor_width),
                caret_height: self.cursor_height,
                caret_radius: self.cursor_radius,
                paragraph_style,
            }),
        )
        .build(cx);

        let mut text_box = IrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        );
        text_box.add_child(text_id);
        let text_layout_id = text_box.build(cx);

        // 3. Scroll Container
        let mut scroll = IrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Scroll {
                direction: if self.multiline && self.wrap_mode != TextWrapMode::NoWrap {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                show_scrollbar: self.multiline
                    && self.show_scrollbar
                    && self.scroll_policy != TextScrollPolicy::Never,
                width: None, // Let it fill parent padding box
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 1.0,
                flex_shrink: 1.0,
            }),
        );
        scroll.add_child(text_layout_id);
        let scroll_id = scroll.build(cx);

        // 4. Editable content row and vertical alignment container.
        let mut content_row = IrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
                wrap: FlexWrap::NoWrap,
                flex_grow: if self.multiline || self.expands {
                    1.0
                } else {
                    0.0
                },
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: if self.prefix.is_some() || self.suffix.is_some() {
                    Some(component_style.gap.unwrap_or(theme.padding_h * 0.75))
                } else {
                    None
                },
                line_gap: None,
                align_items: if self.multiline {
                    fission_ir::op::AlignItems::Stretch
                } else {
                    self.text_align_vertical.align_items(false)
                },
                justify_content: fission_ir::op::JustifyContent::Start,
            }),
        );
        if let Some(prefix) = &self.prefix {
            content_row.add_child(prefix.lower(cx));
        }
        content_row.add_child(scroll_id);
        if let Some(suffix) = &self.suffix {
            content_row.add_child(suffix.lower(cx));
        }
        let content_row_id = content_row.build(cx);

        let mut content_alignment = IrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                wrap: FlexWrap::NoWrap,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: None,
                line_gap: None,
                align_items: fission_ir::op::AlignItems::Stretch,
                justify_content: self.text_align_vertical.justify_content(self.multiline),
            }),
        );
        content_alignment.add_child(content_row_id);
        let content_id = content_alignment.build(cx);

        let effective_line_height = line_height.unwrap_or((font_size * 1.35).max(font_size + 4.0));
        let resolved_height = component_style.height.unwrap_or(theme.height);
        let implicit_single_line_height = resolved_height
            .max(component_style.min_height.unwrap_or(0.0))
            .max(content_padding[2] + content_padding[3] + effective_line_height);
        let min_height = if self.height.is_some() || self.expands {
            None
        } else if self.multiline {
            Some(component_style.min_height.unwrap_or(0.0).max(
                content_padding[2]
                    + content_padding[3]
                    + effective_line_height * self.min_lines.unwrap_or(1) as f32,
            ))
        } else {
            Some(implicit_single_line_height)
        };
        let max_height = if self.height.is_some() || !self.multiline || self.expands {
            None
        } else {
            self.max_lines.map(|lines| {
                content_padding[2] + content_padding[3] + effective_line_height * lines as f32
            })
        };

        // 5. Wrapper (Border + Padding)
        let wrapper_id = cx.next_node_id();
        let mut wrapper = IrBuilder::new(
            wrapper_id,
            Op::Layout(LayoutOp::Box {
                width: self.width.or(component_style.width),
                height: self.height.or(if self.multiline || self.expands {
                    None
                } else {
                    Some(implicit_single_line_height)
                }),
                min_width: None,
                max_width: component_style.max_width,
                min_height,
                max_height,
                padding: content_padding,
                flex_grow: if self.expands { 1.0 } else { 0.0 },
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        );
        // A disabled field fades as a whole, like a disabled button, so it reads as
        // unavailable rather than only a slightly different background.
        let disabled_opacity = (!self.enabled)
            .then_some(component_style.opacity)
            .flatten()
            .filter(|opacity| opacity.is_finite() && *opacity < 1.0);
        if let Some(opacity) = disabled_opacity {
            wrapper = wrapper.composite(fission_ir::CompositeStyle {
                opacity: Some(fission_ir::CompositeScalar::new(opacity.clamp(0.0, 1.0))),
                ..fission_ir::CompositeStyle::default()
            });
        }
        for background_id in background_ids {
            wrapper.add_child(background_id);
        }
        wrapper.add_child(content_id); // Content

        let wrapper_visual_id = wrapper.build(cx);
        let mut final_visual_id = wrapper_visual_id;

        if is_focused && self.enabled {
            if let Some(session_state) = session {
                let affordances = &session_state.affordances;
                let mut overlay_children = Vec::new();

                if self.selection_controls.enabled && affordances.touch_handles {
                    if caret == anchor {
                        if self.selection_controls.show_collapsed_handle {
                            if let Some(point) = affordances.caret_handle {
                                overlay_children.push(self.build_selection_handle_overlay(
                                    cx,
                                    input_id,
                                    TextSelectionHandleKind::Caret,
                                    point,
                                ));
                            }
                        }
                    } else {
                        if let Some(point) = affordances.selection_start_handle {
                            overlay_children.push(self.build_selection_handle_overlay(
                                cx,
                                input_id,
                                TextSelectionHandleKind::Start,
                                point,
                            ));
                        }
                        if let Some(point) = affordances.selection_end_handle {
                            overlay_children.push(self.build_selection_handle_overlay(
                                cx,
                                input_id,
                                TextSelectionHandleKind::End,
                                point,
                            ));
                        }
                    }
                }

                if self.context_menu.enabled && affordances.toolbar_visible {
                    if let Some(anchor_point) = affordances.toolbar_anchor {
                        overlay_children.push(self.build_toolbar_overlay(
                            cx,
                            input_id,
                            anchor_point,
                        ));
                    }
                }

                // A field with an explicit identity lifts its open menu above all content while
                // building. A field without one only has an identity once lowered, so its menu is
                // drawn here, in place.
                if self.id.is_none()
                    && self.context_menu.enabled
                    && !affordances.toolbar_visible
                    && cx.runtime_state.context_menu.owner == Some(input_id)
                {
                    if let Some(menu_anchor) = cx.runtime_state.context_menu.anchor {
                        let local = crate::ui::widgets::context_menu::anchor_to_local(
                            cx,
                            input_id,
                            menu_anchor,
                        );
                        let menu = crate::ui::widgets::context_menu::text_input_menu(
                            input_id,
                            &self.context_menu,
                            local,
                            caret != anchor,
                            !display_text.is_empty(),
                            !self.read_only,
                        );
                        overlay_children.push(menu.lower(cx));
                    }
                }

                if self.magnifier_configuration.enabled && affordances.magnifier_visible {
                    if let Some(anchor_point) = affordances.magnifier_anchor {
                        overlay_children.push(self.build_magnifier_overlay(
                            cx,
                            anchor_point,
                            &display_text,
                            caret.max(anchor),
                            &base_text_style,
                        ));
                    }
                }

                if !overlay_children.is_empty() {
                    let mut stack = IrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::ZStack));
                    stack.add_child(wrapper_visual_id);
                    for child in overlay_children {
                        stack.add_child(child);
                    }
                    final_visual_id = stack.build(cx);
                }
            }
        }

        let supporting_text = self
            .error_text
            .as_ref()
            .map(|text| Self::resolve_text_content(text, cx))
            .or_else(|| {
                self.helper_text
                    .as_ref()
                    .map(|text| Self::resolve_text_content(text, cx))
            });
        let counter_text = self.supporting_counter_text(cx, model_text);

        let field_body_id =
            if resolved_label.is_some() || supporting_text.is_some() || counter_text.is_some() {
                let label_color = self.label_color.unwrap_or(
                    theme
                        .label_style
                        .text_color
                        .unwrap_or(tokens.colors.text_primary),
                );
                let supporting_color = if is_invalid {
                    self.error_color.unwrap_or(tokens.colors.error)
                } else {
                    self.helper_color.unwrap_or(
                        theme
                            .helper_style
                            .text_color
                            .unwrap_or(tokens.colors.text_secondary),
                    )
                };
                let counter_color = self.counter_color.unwrap_or(
                    theme
                        .helper_style
                        .text_color
                        .unwrap_or(tokens.colors.text_secondary),
                );
                let mut column = IrBuilder::new(
                    cx.next_node_id(),
                    Op::Layout(LayoutOp::Flex {
                        direction: FlexDirection::Column,
                        wrap: FlexWrap::NoWrap,
                        flex_grow: 0.0,
                        flex_shrink: 1.0,
                        padding: [0.0; 4],
                        gap: Some(6.0),
                        line_gap: None,
                        align_items: fission_ir::op::AlignItems::Stretch,
                        justify_content: fission_ir::op::JustifyContent::Start,
                    }),
                );

                if let Some(label) = &resolved_label {
                    column.add_child(
                        Text::new(label.clone())
                            .size(
                                theme
                                    .label_style
                                    .font_size
                                    .unwrap_or(tokens.typography.label_large_size),
                            )
                            .weight(
                                theme
                                    .label_style
                                    .font_weight
                                    .unwrap_or(tokens.typography.font_weight_medium),
                            )
                            .color(label_color)
                            .lower(cx),
                    );
                }

                column.add_child(final_visual_id);

                if supporting_text.is_some() || counter_text.is_some() {
                    let mut row = Row::default().gap(8.0);
                    // The message takes the row's free width and wraps inside the field;
                    // at its natural width a long message ran past the field's edge.
                    let has_message = supporting_text.is_some();
                    if let Some(supporting_text) = supporting_text {
                        row.children.push(
                            Text::new(supporting_text)
                                .size(
                                    theme
                                        .helper_style
                                        .font_size
                                        .unwrap_or(tokens.typography.label_large_size),
                                )
                                .color(supporting_color)
                                .flex_grow(1.0)
                                .flex_shrink(1.0)
                                .into(),
                        );
                    }
                    if !has_message {
                        row.children.push(
                            Spacer {
                                flex_grow: 1.0,
                                ..Default::default()
                            }
                            .into(),
                        );
                    }
                    if let Some(counter_text) = counter_text {
                        row.children.push(
                            Text::new(counter_text)
                                .size(
                                    theme
                                        .helper_style
                                        .font_size
                                        .unwrap_or(tokens.typography.label_large_size),
                                )
                                .color(counter_color)
                                .into(),
                        );
                    }
                    column.add_child(row.lower(cx));
                }

                column.build(cx)
            } else {
                final_visual_id
            };

        // 5. Semantics
        let spell_check_enabled = self
            .spell_check_configuration
            .as_ref()
            .map_or(self.spell_check, |cfg| cfg.enabled);
        let suggestions_enabled = self
            .spell_check_configuration
            .as_ref()
            .map_or(self.enable_suggestions, |cfg| {
                self.enable_suggestions && cfg.show_suggestions
            });
        let mut semantics = self.semantics.clone().unwrap_or_default();
        if semantics.role == Role::Generic {
            semantics.role = Role::TextInput;
        }
        semantics.text_editable = true;
        if semantics.label.is_none() {
            semantics.label = resolved_label.clone().or(resolved_placeholder.clone());
        }
        if let Some(identifier) = &self.semantics_identifier {
            semantics.identifier = Some(identifier.clone());
        }
        semantics.value = Some(semantic_value);
        semantics.focusable = self.enabled && self.can_request_focus;
        semantics.sequential_focusable = self.enabled && self.can_request_focus;
        semantics.multiline = self.multiline;
        semantics.text_wrap_mode = self.wrap_mode;
        semantics.masked = self.obscure_text;
        semantics.input_mask = self.mask.clone();
        semantics.ime_preedit_range = preedit_range;
        semantics.ime_preedit_cursor_range = preedit_cursor_range;
        semantics.text_selection = Some((anchor, caret));
        semantics.context_menu = self.context_menu.enabled && self.enabled;
        semantics.disabled = !self.enabled;
        semantics.read_only = self.read_only;
        semantics.autofocus = self.autofocus;
        semantics.text_input_type = if self.multiline {
            TextInputType::Multiline
        } else {
            self.keyboard_type
        };
        semantics.text_input_action = self.text_input_action;
        semantics.text_capitalization = self.text_capitalization;
        semantics.max_length = self.max_length;
        semantics.max_length_enforcement = self.max_length_enforcement;
        semantics.input_formatters = self.input_formatters.clone();
        semantics.text_field_name = self.name.clone();
        semantics.text_form_id = self.form_id.clone();
        semantics.autofill_group = self.autofill_group.clone();
        semantics.required = effective_required;
        semantics.min_length = self.min_length;
        semantics.validation_pattern = self.validation_pattern.clone();
        // Required fields participate in validation immediately, but an untouched empty field is
        // not yet presented as an error. Keep the semantic state aligned with that presentation so
        // assistive technology and native HTML do not announce a new form as already invalid.
        semantics.validation_state = if is_invalid && !show_invalid {
            TextFieldValidationState::Unvalidated
        } else {
            effective_validation_state
        };
        semantics.validation_message = effective_validation_message;
        semantics.autocorrect = self.autocorrect;
        semantics.enable_suggestions = suggestions_enabled;
        semantics.spell_check = spell_check_enabled;
        semantics.smart_dashes = self.smart_dashes;
        semantics.smart_quotes = self.smart_quotes;
        semantics.autofill_hints = self.autofill_hints.clone();
        semantics.scroll_padding = self.scroll_padding;
        semantics.capture_tab = self.capture_tab;
        semantics.auto_indent = self.auto_indent;
        if let Some(env) = &self.on_tab {
            for modifiers in [0, fission_ir::MOD_SHIFT] {
                semantics.key_actions.push(
                    fission_ir::KeyAction::with_modifiers(
                        fission_ir::KeyCode::Tab,
                        modifiers,
                        env.id.as_u128(),
                    )
                    .payload(env.payload.clone()),
                );
            }
        }
        if let Some(env) = &self.on_input {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::TextChanged,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(env) = &self.on_cursor_change {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::CursorChange,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(env) = &self.on_submit {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Submit,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(env) = &self.on_editing_complete {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::EditingComplete,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(env) = &self.on_tap_outside {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::TapOutside,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(env) = &self.on_focus {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Focus,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(env) = &self.on_blur {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Blur,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(env) = &self.on_validation {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Validation,
                action_id: env.id.as_u128(),
                payload_data: Some(env.payload.clone()),
            });
        }
        if let Some(mouse_cursor) = self.mouse_cursor {
            semantics
                .actions
                .entries
                .push(fission_ir::ActionEntry::hover_cursor(mouse_cursor));
        }
        let mut semantics_builder = IrBuilder::new(input_id, Op::Semantics(semantics));
        semantics_builder.add_child(field_body_id);
        let semantics_id = semantics_builder.build(cx);
        cx.ir.custom_render_objects.insert(
            semantics_id,
            Arc::new(TextInputRuntimeConfig {
                drag_start_behavior: self.drag_start_behavior,
                undo_controller: self.undo_controller.clone(),
                restoration_id: self.restoration_id.clone(),
                placeholder: resolved_placeholder.clone(),
                spell_check_configuration: self.spell_check_configuration.clone(),
                custom_input_formatters: self.custom_input_formatters.clone(),
                select_all_on_focus: self.select_all_on_focus,
                scroll_policy: self.scroll_policy,
                scroll_physics: self.scroll_physics,
                form_id: self.form_id.clone(),
                validator: self.validator.clone(),
            }),
        );
        semantics_id
    }
}
