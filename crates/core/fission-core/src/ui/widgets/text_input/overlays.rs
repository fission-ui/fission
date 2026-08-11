use super::*;

impl TextInput {
    pub(super) fn resolve_text_content(
        content: &TextContent,
        cx: &InternalLoweringCx<'_>,
    ) -> String {
        match content {
            TextContent::Literal(s) => s.clone(),
            TextContent::Key(key) => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("MISSING:{}", key)),
            TextContent::KeyWithFallback { key, fallback } => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| fallback.clone()),
        }
    }

    pub(super) fn mask_text(text: &str, obscuring_character: char) -> String {
        let mut masked = String::new();
        for _ in text.graphemes(true) {
            masked.push(obscuring_character);
        }
        masked
    }

    pub(super) fn masked_byte_offset(
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

    pub(super) fn supporting_counter_text(
        &self,
        cx: &InternalLoweringCx<'_>,
        current_text: &str,
    ) -> Option<String> {
        self.counter_text
            .as_ref()
            .map(|content| Self::resolve_text_content(content, cx))
            .or_else(|| {
                self.max_length
                    .map(|max_length| format!("{}/{}", current_text.chars().count(), max_length))
            })
    }

    pub(super) fn build_selection_handle_overlay(
        &self,
        cx: &mut InternalLoweringCx,
        input_id: WidgetId,
        kind: TextSelectionHandleKind,
        point: fission_layout::LayoutPoint,
    ) -> WidgetId {
        let controls = &self.selection_controls;
        let diameter = controls.handle_radius * 2.0;
        let handle_node = Button {
            id: Some(text_input_selection_handle_id(input_id, kind).into()),
            semantics: Some(Semantics {
                role: Role::Generic,
                draggable: true,
                ..Semantics::default()
            }),
            child: Some(
                Container::new(Spacer {
                    width: Some(diameter),
                    height: Some(diameter),
                    ..Default::default()
                })
                .bg_fill(Fill::Solid(controls.handle_fill))
                .border(
                    controls.handle_stroke.unwrap_or(IrColor {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    }),
                    controls.handle_stroke_width,
                )
                .border_radius(controls.handle_radius)
                .into(),
            ),
            width: Some(diameter),
            height: Some(diameter),
            padding: Some([0.0; 4]),
            content_align: ButtonContentAlign::Center,
            variant: ButtonVariant::Ghost,
            ..Default::default()
        }
        .into();

        Positioned {
            left: Some((point.x - controls.handle_radius).max(0.0)),
            top: Some((point.y - controls.handle_radius).max(0.0)),
            width: Some(diameter),
            height: Some(diameter),
            child: Some(handle_node),
            ..Default::default()
        }
        .lower(cx)
    }

    pub(super) fn build_toolbar_overlay(
        &self,
        cx: &mut InternalLoweringCx,
        input_id: WidgetId,
        anchor: fission_layout::LayoutPoint,
    ) -> WidgetId {
        let tokens = &cx.env.theme.tokens;
        let mut row = Row::default().gap(self.context_menu.menu.gap);
        for action in &self.context_menu.actions {
            row.children.push(
                Button {
                    id: Some(text_input_toolbar_button_id(input_id, *action).into()),
                    semantics: Some(Semantics {
                        role: Role::Button,
                        label: Some(action.fallback_label().into()),
                        focusable: true,
                        focus_policy: fission_ir::FocusPolicy::PreserveCurrentOnPointer,
                        ..Semantics::default()
                    }),
                    focus_policy: fission_ir::FocusPolicy::PreserveCurrentOnPointer,
                    child: Some(
                        Text::new(TextContent::KeyWithFallback {
                            key: action.label_key().to_string(),
                            fallback: action.fallback_label().to_string(),
                        })
                        .size(tokens.typography.label_large_size)
                        .color(tokens.colors.text_primary)
                        .into(),
                    ),
                    padding: Some([10.0, 10.0, 6.0, 6.0]),
                    content_align: ButtonContentAlign::Center,
                    variant: ButtonVariant::Ghost,
                    ..Default::default()
                }
                .into(),
            );
        }

        let toolbar: Widget = Container::new(row)
            .bg_fill(Fill::Solid(tokens.colors.surface))
            .border(tokens.colors.border, 1.0)
            .border_radius(self.context_menu.menu.border_radius)
            .padding(self.context_menu.menu.padding)
            .into();

        Positioned {
            left: Some(anchor.x.max(0.0)),
            top: Some((anchor.y - 44.0).max(0.0)),
            child: Some(toolbar),
            ..Default::default()
        }
        .lower(cx)
    }

    pub(super) fn magnifier_snippet(display_text: &str, caret: usize) -> String {
        let mut graphemes = Vec::new();
        for (idx, grapheme) in display_text.grapheme_indices(true) {
            graphemes.push((idx, grapheme));
        }
        if graphemes.is_empty() {
            return String::new();
        }

        let caret_grapheme = graphemes
            .iter()
            .position(|(idx, _)| *idx >= caret.min(display_text.len()))
            .unwrap_or(graphemes.len().saturating_sub(1));
        let start = caret_grapheme.saturating_sub(4);
        let end = (caret_grapheme + 5).min(graphemes.len());
        graphemes[start..end]
            .iter()
            .map(|(_, grapheme)| *grapheme)
            .collect::<String>()
    }

    pub(super) fn build_magnifier_overlay(
        &self,
        cx: &mut InternalLoweringCx,
        anchor: fission_layout::LayoutPoint,
        display_text: &str,
        caret: usize,
        base_text_style: &fission_ir::op::TextStyle,
    ) -> WidgetId {
        let cfg = &self.magnifier_configuration;
        let tokens = &cx.env.theme.tokens;
        let preview = Self::magnifier_snippet(display_text, caret);
        let preview_text = Text::new(preview)
            .size(base_text_style.font_size * cfg.scale)
            .color(base_text_style.color)
            .family(
                base_text_style
                    .font_family
                    .clone()
                    .unwrap_or_else(|| "system-ui".to_string()),
            )
            .weight(base_text_style.font_weight)
            .italic(base_text_style.font_style == fission_ir::op::FontStyle::Italic)
            .line_height(
                base_text_style
                    .line_height
                    .unwrap_or(base_text_style.font_size * 1.25)
                    * cfg.scale,
            )
            .letter_spacing(base_text_style.letter_spacing * cfg.scale);

        let magnifier: Widget = Container::new(preview_text)
            .width(cfg.diameter)
            .height(cfg.diameter)
            .bg_fill(Fill::Solid(tokens.colors.surface))
            .border(
                cfg.border_color.unwrap_or(tokens.colors.border),
                cfg.border_width,
            )
            .border_radius(cfg.border_radius)
            .padding_all(8.0)
            .into();

        Positioned {
            left: Some((anchor.x - cfg.diameter * 0.5).max(0.0)),
            top: Some((anchor.y - cfg.diameter - 18.0).max(0.0)),
            width: Some(cfg.diameter),
            height: Some(cfg.diameter),
            child: Some(magnifier),
            ..Default::default()
        }
        .lower(cx)
    }
}
