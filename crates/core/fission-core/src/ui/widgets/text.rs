use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ActionEnvelope;
use fission_ir::{
    op::{
        decode_inline_widget_marker, encode_inline_widget_marker, Color as IrColor,
        FontStyle as IrFontStyle, LayoutOp, MouseCursor as IrMouseCursor, Op, PaintOp,
        RichTextAnnotation as IrRichTextAnnotation, TextAlign as IrTextAlign,
        TextDirection as IrTextDirection, TextHeightBehavior as IrTextHeightBehavior,
        TextOverflow as IrTextOverflow, TextParagraphStyle as IrTextParagraphStyle,
        TextRun as IrTextRun, TextWidthBasis as IrTextWidthBasis,
    },
    semantics::ActionTrigger,
    ActionEntry, CompositeStyle, Role, Semantics, WidgetId,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The content source for a [`Text`] widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextContent {
    Literal(String),
    Key(String),
}

impl From<&str> for TextContent {
    fn from(value: &str) -> Self {
        TextContent::Literal(value.to_string())
    }
}

impl From<String> for TextContent {
    fn from(value: String) -> Self {
        TextContent::Literal(value)
    }
}

impl Default for TextContent {
    fn default() -> Self {
        TextContent::Literal(String::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextFontStyle {
    #[default]
    Normal,
    Italic,
}

impl From<TextFontStyle> for IrFontStyle {
    fn from(value: TextFontStyle) -> Self {
        match value {
            TextFontStyle::Normal => IrFontStyle::Normal,
            TextFontStyle::Italic => IrFontStyle::Italic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TextScaler(f32);

impl TextScaler {
    pub fn linear(scale_factor: f32) -> Self {
        Self(scale_factor)
    }

    pub fn scale_factor(self) -> f32 {
        self.0
    }
}

impl Default for TextScaler {
    fn default() -> Self {
        Self::linear(1.0)
    }
}

impl From<f32> for TextScaler {
    fn from(value: f32) -> Self {
        Self::linear(value)
    }
}

impl From<TextScaler> for f32 {
    fn from(value: TextScaler) -> Self {
        value.scale_factor()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextRunStyle {
    pub font_size: Option<f32>,
    pub color: Option<IrColor>,
    pub underline: bool,
    pub font_family: Option<String>,
    pub locale: Option<String>,
    pub font_weight: Option<u16>,
    pub font_style: TextFontStyle,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub text_scale: Option<f32>,
    pub background_color: Option<IrColor>,
}

impl TextRunStyle {
    fn resolve(
        &self,
        theme: &fission_theme::Theme,
        fallback_size: Option<f32>,
        fallback_color: Option<IrColor>,
    ) -> fission_ir::op::TextStyle {
        let scale = self.text_scale.unwrap_or(1.0).max(0.0);
        let base_font_size = self
            .font_size
            .or(fallback_size)
            .unwrap_or(theme.tokens.typography.body_medium_size);
        let base_line_height = self.line_height.or(Some(base_font_size * 1.2));
        let base_letter_spacing = self.letter_spacing.unwrap_or(0.0);
        fission_ir::op::TextStyle {
            font_size: base_font_size * scale,
            color: self
                .color
                .or(fallback_color)
                .unwrap_or(theme.tokens.colors.text_primary),
            underline: self.underline,
            font_family: self.font_family.clone(),
            locale: self.locale.clone(),
            font_weight: self.font_weight.unwrap_or(400),
            font_style: self.font_style.into(),
            line_height: base_line_height.map(|value| value * scale),
            letter_spacing: base_letter_spacing * scale,
            background_color: self.background_color,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichTextRun {
    pub text: String,
    pub style: TextRunStyle,
    pub semantics_label: Option<String>,
    pub semantics_identifier: Option<String>,
    #[serde(default)]
    pub spell_out: Option<bool>,
}

impl RichTextRun {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextRunStyle::default(),
            semantics_label: None,
            semantics_identifier: None,
            spell_out: None,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.style.font_size = Some(size);
        self
    }

    pub fn color(mut self, color: IrColor) -> Self {
        self.style.color = Some(color);
        self
    }

    pub fn underline(mut self, underline: bool) -> Self {
        self.style.underline = underline;
        self
    }

    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.style.font_family = Some(family.into());
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.style.locale = Some(locale.into());
        self
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.style.font_weight = Some(weight);
        self
    }

    pub fn italic(mut self, italic: bool) -> Self {
        self.style.font_style = if italic {
            TextFontStyle::Italic
        } else {
            TextFontStyle::Normal
        };
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.style.line_height = Some(line_height);
        self
    }

    pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.style.letter_spacing = Some(letter_spacing);
        self
    }

    pub fn text_scale(mut self, text_scale: f32) -> Self {
        self.style.text_scale = Some(text_scale);
        self
    }

    pub fn text_scaler(mut self, text_scaler: impl Into<TextScaler>) -> Self {
        self.style.text_scale = Some(text_scaler.into().scale_factor());
        self
    }

    pub fn background_color(mut self, color: IrColor) -> Self {
        self.style.background_color = Some(color);
        self
    }

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics_label = Some(label.into());
        self
    }

    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    pub fn spell_out(mut self, spell_out: bool) -> Self {
        self.spell_out = Some(spell_out);
        self
    }

    pub fn into_span(self) -> RichTextSpan {
        RichTextSpan::from(self)
    }

    fn lower_with_theme(
        &self,
        theme: &fission_theme::Theme,
        fallback_size: Option<f32>,
        fallback_color: Option<IrColor>,
    ) -> IrTextRun {
        IrTextRun {
            text: self.text.clone(),
            style: self.style.resolve(theme, fallback_size, fallback_color),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RichTextSpanStyle {
    pub font_size: Option<f32>,
    pub color: Option<IrColor>,
    pub underline: Option<bool>,
    pub font_family: Option<String>,
    pub locale: Option<String>,
    pub font_weight: Option<u16>,
    pub font_style: Option<TextFontStyle>,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub text_scale: Option<f32>,
    pub background_color: Option<IrColor>,
}

impl RichTextSpanStyle {
    fn cascade(&self, inherited: &TextRunStyle) -> TextRunStyle {
        TextRunStyle {
            font_size: self.font_size.or(inherited.font_size),
            color: self.color.or(inherited.color),
            underline: self.underline.unwrap_or(inherited.underline),
            font_family: self
                .font_family
                .clone()
                .or_else(|| inherited.font_family.clone()),
            locale: self.locale.clone().or_else(|| inherited.locale.clone()),
            font_weight: self.font_weight.or(inherited.font_weight),
            font_style: self.font_style.unwrap_or(inherited.font_style),
            line_height: self.line_height.or(inherited.line_height),
            letter_spacing: self.letter_spacing.or(inherited.letter_spacing),
            text_scale: self.text_scale.or(inherited.text_scale),
            background_color: self.background_color.or(inherited.background_color),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RichTextSpan {
    pub text: String,
    pub style: RichTextSpanStyle,
    pub children: Vec<RichTextChild>,
    pub semantics_label: Option<String>,
    pub semantics_identifier: Option<String>,
    #[serde(default)]
    pub spell_out: Option<bool>,
    #[serde(default)]
    pub mouse_cursor: Option<IrMouseCursor>,
    #[serde(default)]
    pub actions: Vec<ActionEntry>,
}

pub type TextSpan = RichTextSpan;
pub type WidgetSpan = InlineWidgetSpan;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineWidgetSpan {
    pub widget: crate::ui::Widget,
    pub width: f32,
    pub height: f32,
    pub semantics_label: Option<String>,
}

impl PartialEq for InlineWidgetSpan {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width
            && self.height == other.height
            && self.semantics_label == other.semantics_label
            && serde_json::to_vec(&self.widget).ok() == serde_json::to_vec(&other.widget).ok()
    }
}

impl InlineWidgetSpan {
    pub fn new(widget: impl Into<crate::ui::Widget>, width: f32, height: f32) -> Self {
        Self {
            widget: widget.into(),
            width,
            height,
            semantics_label: None,
        }
    }

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics_label = Some(label.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RichTextChild {
    Span(RichTextSpan),
    Widget(InlineWidgetSpan),
}

impl RichTextSpan {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.style.font_size = Some(size);
        self
    }

    pub fn color(mut self, color: IrColor) -> Self {
        self.style.color = Some(color);
        self
    }

    pub fn underline(mut self, underline: bool) -> Self {
        self.style.underline = Some(underline);
        self
    }

    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.style.font_family = Some(family.into());
        self
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.style.font_weight = Some(weight);
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.style.locale = Some(locale.into());
        self
    }

    pub fn italic(mut self, italic: bool) -> Self {
        self.style.font_style = Some(if italic {
            TextFontStyle::Italic
        } else {
            TextFontStyle::Normal
        });
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.style.line_height = Some(line_height);
        self
    }

    pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.style.letter_spacing = Some(letter_spacing);
        self
    }

    pub fn text_scale(mut self, text_scale: f32) -> Self {
        self.style.text_scale = Some(text_scale);
        self
    }

    pub fn text_scaler(mut self, text_scaler: impl Into<TextScaler>) -> Self {
        self.style.text_scale = Some(text_scaler.into().scale_factor());
        self
    }

    pub fn background_color(mut self, color: IrColor) -> Self {
        self.style.background_color = Some(color);
        self
    }

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics_label = Some(label.into());
        self
    }

    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    pub fn spell_out(mut self, spell_out: bool) -> Self {
        self.spell_out = Some(spell_out);
        self
    }

    pub fn mouse_cursor(mut self, mouse_cursor: IrMouseCursor) -> Self {
        self.mouse_cursor = Some(mouse_cursor);
        self
    }

    pub fn on_tap(mut self, action: ActionEnvelope) -> Self {
        upsert_action_entry(&mut self.actions, ActionTrigger::Default, &action);
        self
    }

    pub fn on_hover_enter(mut self, action: ActionEnvelope) -> Self {
        upsert_action_entry(&mut self.actions, ActionTrigger::HoverEnter, &action);
        self
    }

    pub fn on_hover_exit(mut self, action: ActionEnvelope) -> Self {
        upsert_action_entry(&mut self.actions, ActionTrigger::HoverExit, &action);
        self
    }

    pub fn on_secondary_click(mut self, action: ActionEnvelope) -> Self {
        upsert_action_entry(&mut self.actions, ActionTrigger::SecondaryClick, &action);
        self
    }

    pub fn children<I, T>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<RichTextChild>,
    {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    fn push_runs(
        &self,
        inherited: &TextRunStyle,
        runs: &mut Vec<RichTextRun>,
        inline_widgets: &mut Vec<InlineWidgetSpan>,
        annotations: &mut Vec<IrRichTextAnnotation>,
        byte_cursor: &mut usize,
    ) {
        let style = self.style.cascade(inherited);
        let span_start = *byte_cursor;
        push_rich_text_run(runs, &self.text, &style);
        *byte_cursor += self.text.len();
        for child in &self.children {
            match child {
                RichTextChild::Span(child) => {
                    child.push_runs(&style, runs, inline_widgets, annotations, byte_cursor)
                }
                RichTextChild::Widget(widget) => {
                    let inline_id = inline_widgets.len() as u64;
                    inline_widgets.push(widget.clone());
                    runs.push(RichTextRun {
                        text: String::new(),
                        style: TextRunStyle {
                            font_size: style.font_size,
                            color: Some(IrColor {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 0,
                            }),
                            underline: false,
                            font_family: Some(encode_inline_widget_marker(
                                inline_id,
                                widget.width,
                                widget.height,
                            )),
                            locale: style.locale.clone(),
                            font_weight: style.font_weight,
                            font_style: style.font_style,
                            line_height: style.line_height,
                            letter_spacing: style.letter_spacing,
                            text_scale: style.text_scale,
                            background_color: None,
                        },
                        semantics_label: None,
                        semantics_identifier: None,
                        spell_out: None,
                    });
                }
            }
        }
        let span_end = *byte_cursor;
        if let Some(annotation) = self.annotation(span_start..span_end) {
            annotations.push(annotation);
        }
    }

    fn collect_semantics_text(&self, out: &mut String) -> bool {
        let mut has_override = false;
        if let Some(label) = &self.semantics_label {
            out.push_str(label);
            has_override = true;
        } else {
            out.push_str(&self.text);
        }
        for child in &self.children {
            match child {
                RichTextChild::Span(child) => {
                    has_override |= child.collect_semantics_text(out);
                }
                RichTextChild::Widget(widget) => {
                    if let Some(label) = &widget.semantics_label {
                        out.push_str(label);
                        has_override = true;
                    }
                }
            }
        }
        has_override
    }

    fn collect_semantics_identifier(&self) -> Option<String> {
        if let Some(identifier) = &self.semantics_identifier {
            return Some(identifier.clone());
        }
        for child in &self.children {
            if let RichTextChild::Span(child) = child {
                if let Some(identifier) = child.collect_semantics_identifier() {
                    return Some(identifier);
                }
            }
        }
        None
    }

    fn annotation(&self, range: std::ops::Range<usize>) -> Option<IrRichTextAnnotation> {
        if range.start >= range.end
            || (self.semantics_label.is_none()
                && self.semantics_identifier.is_none()
                && self.spell_out.is_none()
                && self.mouse_cursor.is_none()
                && self.actions.is_empty())
        {
            return None;
        }

        Some(IrRichTextAnnotation {
            range,
            semantics_label: self.semantics_label.clone(),
            semantics_identifier: self.semantics_identifier.clone(),
            spell_out: self.spell_out,
            mouse_cursor: self.mouse_cursor,
            actions: self.actions.clone(),
        })
    }
}

impl From<RichTextRun> for RichTextSpan {
    fn from(value: RichTextRun) -> Self {
        Self {
            text: value.text,
            style: RichTextSpanStyle {
                font_size: value.style.font_size,
                color: value.style.color,
                underline: Some(value.style.underline),
                font_family: value.style.font_family,
                locale: value.style.locale,
                font_weight: value.style.font_weight,
                font_style: Some(value.style.font_style),
                line_height: value.style.line_height,
                letter_spacing: value.style.letter_spacing,
                text_scale: value.style.text_scale,
                background_color: value.style.background_color,
            },
            children: Vec::new(),
            semantics_label: value.semantics_label,
            semantics_identifier: value.semantics_identifier,
            spell_out: value.spell_out,
            mouse_cursor: None,
            actions: Vec::new(),
        }
    }
}

impl From<RichTextRun> for RichTextChild {
    fn from(value: RichTextRun) -> Self {
        Self::Span(value.into())
    }
}

impl From<RichTextSpan> for RichTextChild {
    fn from(value: RichTextSpan) -> Self {
        Self::Span(value)
    }
}

impl From<InlineWidgetSpan> for RichTextChild {
    fn from(value: InlineWidgetSpan) -> Self {
        Self::Widget(value)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Text {
    pub id: Option<WidgetId>,
    pub content: TextContent,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub font_size: Option<f32>,
    pub color: Option<IrColor>,
    pub underline: bool,
    pub font_family: Option<String>,
    pub font_weight: Option<u16>,
    pub font_style: TextFontStyle,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub locale: Option<String>,
    pub text_scale: Option<f32>,
    pub wrap: bool,
    pub text_align: IrTextAlign,
    pub text_direction: IrTextDirection,
    pub text_width_basis: IrTextWidthBasis,
    pub max_lines: Option<usize>,
    pub overflow: IrTextOverflow,
    pub strut_line_height: Option<f32>,
    pub text_height_behavior: IrTextHeightBehavior,
    pub selection_range: Option<(usize, usize)>,
    pub selection_color: Option<IrColor>,
    pub selection_text_color: Option<IrColor>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Text {
    pub fn new(content: impl Into<TextContent>) -> Self {
        Self {
            content: content.into(),
            wrap: true,
            ..Default::default()
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }

    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = Some(w);
        self
    }

    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }

    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = Some(h);
        self
    }

    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = Some(h);
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow;
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = shrink;
        self
    }

    pub fn color(mut self, color: IrColor) -> Self {
        self.color = Some(color);
        self
    }

    pub fn underline(mut self, u: bool) -> Self {
        self.underline = u;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.font_weight = Some(weight);
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    pub fn italic(mut self, italic: bool) -> Self {
        self.font_style = if italic {
            TextFontStyle::Italic
        } else {
            TextFontStyle::Normal
        };
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = Some(line_height);
        self
    }

    pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = Some(letter_spacing);
        self
    }

    pub fn text_scale(mut self, text_scale: f32) -> Self {
        self.text_scale = Some(text_scale);
        self
    }

    pub fn text_scaler(mut self, text_scaler: impl Into<TextScaler>) -> Self {
        self.text_scale = Some(text_scaler.into().scale_factor());
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn text_align(mut self, text_align: IrTextAlign) -> Self {
        self.text_align = text_align;
        self
    }

    pub fn text_direction(mut self, text_direction: IrTextDirection) -> Self {
        self.text_direction = text_direction;
        self
    }

    pub fn text_width_basis(mut self, text_width_basis: IrTextWidthBasis) -> Self {
        self.text_width_basis = text_width_basis;
        self
    }

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines);
        self
    }

    pub fn overflow(mut self, overflow: IrTextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn strut_line_height(mut self, line_height: f32) -> Self {
        self.strut_line_height = Some(line_height);
        self
    }

    pub fn text_height_behavior(mut self, behavior: IrTextHeightBehavior) -> Self {
        self.text_height_behavior = behavior;
        self
    }

    pub fn selection_range(mut self, range: (usize, usize)) -> Self {
        self.selection_range = Some(range);
        self
    }

    pub fn selection_color(mut self, color: IrColor) -> Self {
        self.selection_color = Some(color);
        self
    }

    pub fn selection_text_color(mut self, color: IrColor) -> Self {
        self.selection_text_color = Some(color);
        self
    }

    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        let mut semantics = self.semantics.take().unwrap_or_default();
        semantics.identifier = Some(identifier.into());
        self.semantics = Some(semantics);
        self
    }

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics = Some(merge_semantics_label(self.semantics.take(), label));
        self
    }

    pub fn on_tap(mut self, action: ActionEnvelope) -> Self {
        self.semantics = Some(merge_semantics_action(
            self.semantics.take(),
            ActionTrigger::Default,
            action,
        ));
        self
    }

    pub fn on_hover_enter(mut self, action: ActionEnvelope) -> Self {
        self.semantics = Some(merge_semantics_action(
            self.semantics.take(),
            ActionTrigger::HoverEnter,
            action,
        ));
        self
    }

    pub fn on_hover_exit(mut self, action: ActionEnvelope) -> Self {
        self.semantics = Some(merge_semantics_action(
            self.semantics.take(),
            ActionTrigger::HoverExit,
            action,
        ));
        self
    }

    pub fn on_secondary_click(mut self, action: ActionEnvelope) -> Self {
        self.semantics = Some(merge_semantics_action(
            self.semantics.take(),
            ActionTrigger::SecondaryClick,
            action,
        ));
        self
    }

    fn resolve_text(&self, cx: &InternalLoweringCx<'_>) -> String {
        match &self.content {
            TextContent::Literal(s) => s.clone(),
            TextContent::Key(key) => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("MISSING:{}", key)),
        }
    }

    fn resolved_style(&self, cx: &InternalLoweringCx<'_>) -> fission_ir::op::TextStyle {
        let scale = self.text_scale.unwrap_or(1.0).max(0.0);
        let base_font_size = self
            .font_size
            .unwrap_or(cx.env.theme.tokens.typography.body_medium_size);
        fission_ir::op::TextStyle {
            font_size: base_font_size * scale,
            color: self
                .color
                .unwrap_or(cx.env.theme.tokens.colors.text_primary),
            underline: self.underline,
            font_family: self.font_family.clone(),
            locale: self.locale.clone(),
            font_weight: self.font_weight.unwrap_or(400),
            font_style: self.font_style.into(),
            line_height: Some(self.line_height.unwrap_or(base_font_size * 1.2) * scale),
            letter_spacing: self.letter_spacing.unwrap_or(0.0) * scale,
            background_color: None,
        }
    }

    fn needs_rich_text(&self) -> bool {
        self.font_family.is_some()
            || self.locale.is_some()
            || self.font_weight.is_some()
            || self.font_style != TextFontStyle::Normal
            || self.line_height.is_some()
            || self.letter_spacing.unwrap_or(0.0) != 0.0
            || self.text_scale.unwrap_or(1.0) != 1.0
            || self.selection_range.is_some()
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RichText {
    pub id: Option<WidgetId>,
    pub runs: Vec<RichTextRun>,
    pub inline_widgets: Vec<InlineWidgetSpan>,
    #[serde(default)]
    pub annotations: Vec<IrRichTextAnnotation>,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub wrap: bool,
    pub text_align: IrTextAlign,
    pub text_direction: IrTextDirection,
    pub text_width_basis: IrTextWidthBasis,
    pub max_lines: Option<usize>,
    pub overflow: IrTextOverflow,
    pub strut_line_height: Option<f32>,
    pub text_height_behavior: IrTextHeightBehavior,
    pub selection_range: Option<(usize, usize)>,
    pub selection_color: Option<IrColor>,
    pub selection_text_color: Option<IrColor>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl RichText {
    pub fn new(runs: Vec<RichTextRun>) -> Self {
        if runs.iter().any(|run| {
            run.semantics_label.is_some()
                || run.semantics_identifier.is_some()
                || run.spell_out.is_some()
        }) {
            return Self::from_spans(runs);
        }

        Self {
            runs,
            inline_widgets: Vec::new(),
            wrap: true,
            ..Default::default()
        }
    }

    pub fn from_span<T>(span: T) -> Self
    where
        T: Into<RichTextChild>,
    {
        Self::from_spans(std::iter::once(span))
    }

    pub fn from_spans<I, T>(spans: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<RichTextChild>,
    {
        let spans: Vec<_> = spans.into_iter().map(Into::into).collect();
        let mut runs = Vec::new();
        let mut inline_widgets = Vec::new();
        let mut annotations = Vec::new();
        let mut semantics_text = String::new();
        let mut has_semantics_override = false;
        let mut semantics_identifier = None;
        let mut byte_cursor = 0usize;

        for span in &spans {
            match span {
                RichTextChild::Span(span) => {
                    span.push_runs(
                        &TextRunStyle::default(),
                        &mut runs,
                        &mut inline_widgets,
                        &mut annotations,
                        &mut byte_cursor,
                    );
                    has_semantics_override |= span.collect_semantics_text(&mut semantics_text);
                    if semantics_identifier.is_none() {
                        semantics_identifier = span.collect_semantics_identifier();
                    }
                }
                RichTextChild::Widget(widget) => {
                    let inline_id = inline_widgets.len() as u64;
                    inline_widgets.push(widget.clone());
                    runs.push(RichTextRun {
                        text: String::new(),
                        style: TextRunStyle {
                            font_size: None,
                            color: Some(IrColor {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 0,
                            }),
                            underline: false,
                            font_family: Some(encode_inline_widget_marker(
                                inline_id,
                                widget.width,
                                widget.height,
                            )),
                            locale: None,
                            font_weight: None,
                            font_style: TextFontStyle::Normal,
                            line_height: None,
                            letter_spacing: None,
                            text_scale: None,
                            background_color: None,
                        },
                        semantics_label: None,
                        semantics_identifier: None,
                        spell_out: None,
                    });
                    if let Some(label) = &widget.semantics_label {
                        semantics_text.push_str(label);
                        has_semantics_override = true;
                    }
                }
            }
        }

        let mut rich_text = Self {
            runs,
            inline_widgets,
            annotations,
            wrap: true,
            ..Default::default()
        };
        if let Some(identifier) = semantics_identifier {
            rich_text = rich_text.semantics_identifier(identifier);
        }
        if has_semantics_override {
            rich_text.semantics = Some(merge_semantics_label(
                rich_text.semantics.take(),
                semantics_text,
            ));
        }
        rich_text
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }

    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = Some(w);
        self
    }

    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }

    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = Some(h);
        self
    }

    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = Some(h);
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow;
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = shrink;
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn text_align(mut self, text_align: IrTextAlign) -> Self {
        self.text_align = text_align;
        self
    }

    pub fn text_direction(mut self, text_direction: IrTextDirection) -> Self {
        self.text_direction = text_direction;
        self
    }

    pub fn text_width_basis(mut self, text_width_basis: IrTextWidthBasis) -> Self {
        self.text_width_basis = text_width_basis;
        self
    }

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines);
        self
    }

    pub fn overflow(mut self, overflow: IrTextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn strut_line_height(mut self, line_height: f32) -> Self {
        self.strut_line_height = Some(line_height);
        self
    }

    pub fn text_height_behavior(mut self, behavior: IrTextHeightBehavior) -> Self {
        self.text_height_behavior = behavior;
        self
    }

    pub fn selection_range(mut self, range: (usize, usize)) -> Self {
        self.selection_range = Some(range);
        self
    }

    pub fn selection_color(mut self, color: IrColor) -> Self {
        self.selection_color = Some(color);
        self
    }

    pub fn selection_text_color(mut self, color: IrColor) -> Self {
        self.selection_text_color = Some(color);
        self
    }

    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        let mut semantics = self.semantics.take().unwrap_or_default();
        semantics.identifier = Some(identifier.into());
        self.semantics = Some(semantics);
        self
    }

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics = Some(merge_semantics_label(self.semantics.take(), label));
        self
    }

    pub fn on_tap(mut self, action: ActionEnvelope) -> Self {
        self.semantics = Some(merge_semantics_action(
            self.semantics.take(),
            ActionTrigger::Default,
            action,
        ));
        self
    }

    pub fn on_hover_enter(mut self, action: ActionEnvelope) -> Self {
        self.semantics = Some(merge_semantics_action(
            self.semantics.take(),
            ActionTrigger::HoverEnter,
            action,
        ));
        self
    }

    pub fn on_hover_exit(mut self, action: ActionEnvelope) -> Self {
        self.semantics = Some(merge_semantics_action(
            self.semantics.take(),
            ActionTrigger::HoverExit,
            action,
        ));
        self
    }

    pub fn on_secondary_click(mut self, action: ActionEnvelope) -> Self {
        self.semantics = Some(merge_semantics_action(
            self.semantics.take(),
            ActionTrigger::SecondaryClick,
            action,
        ));
        self
    }

    fn lower_runs(&self, cx: &InternalLoweringCx<'_>) -> Vec<IrTextRun> {
        self.runs
            .iter()
            .map(|run| run.lower_with_theme(&cx.env.theme, None, None))
            .collect()
    }
}

fn push_rich_text_run(runs: &mut Vec<RichTextRun>, text: &str, style: &TextRunStyle) {
    if text.is_empty() {
        return;
    }

    if let Some(last) = runs.last_mut() {
        if last.style == *style {
            last.text.push_str(text);
            return;
        }
    }

    runs.push(RichTextRun {
        text: text.to_string(),
        style: style.clone(),
        semantics_label: None,
        semantics_identifier: None,
        spell_out: None,
    });
}

fn apply_selection_to_runs(
    runs: Vec<IrTextRun>,
    selection_range: Option<(usize, usize)>,
    selection_color: Option<IrColor>,
    selection_text_color: Option<IrColor>,
) -> Vec<IrTextRun> {
    let Some((start, end)) = selection_range.map(|(start, end)| (start.min(end), start.max(end)))
    else {
        return runs;
    };
    if start == end {
        return runs;
    }

    let selection_fill = selection_color.unwrap_or(IrColor {
        r: 38,
        g: 132,
        b: 255,
        a: 64,
    });

    let mut out = Vec::new();
    let mut byte_cursor = 0usize;

    for run in runs {
        let run_start = byte_cursor;
        let run_end = run_start + run.text.len();
        byte_cursor = run_end;

        if end <= run_start || start >= run_end {
            out.push(run);
            continue;
        }

        let local_start = start.saturating_sub(run_start).min(run.text.len());
        let local_end = end.saturating_sub(run_start).min(run.text.len());

        if local_start > 0 {
            out.push(IrTextRun {
                text: run.text[..local_start].to_string(),
                style: run.style.clone(),
            });
        }

        if local_end > local_start {
            let mut style = run.style.clone();
            style.background_color = Some(selection_fill);
            if let Some(color) = selection_text_color {
                style.color = color;
            }
            out.push(IrTextRun {
                text: run.text[local_start..local_end].to_string(),
                style,
            });
        }

        if local_end < run.text.len() {
            out.push(IrTextRun {
                text: run.text[local_end..].to_string(),
                style: run.style,
            });
        }
    }

    out
}

fn merge_semantics_label(semantics: Option<Semantics>, label: impl Into<String>) -> Semantics {
    let mut semantics = semantics.unwrap_or_default();
    semantics.label = Some(label.into());
    semantics
}

fn merge_semantics_action(
    semantics: Option<Semantics>,
    trigger: ActionTrigger,
    action: ActionEnvelope,
) -> Semantics {
    let mut semantics = semantics.unwrap_or_default();
    upsert_semantics_action(&mut semantics, trigger, &action);
    semantics
}

fn upsert_semantics_action(
    semantics: &mut Semantics,
    trigger: ActionTrigger,
    action: &ActionEnvelope,
) {
    upsert_action_entry(&mut semantics.actions.entries, trigger, action);
}

fn upsert_action_entry(
    entries: &mut Vec<ActionEntry>,
    trigger: ActionTrigger,
    action: &ActionEnvelope,
) {
    entries.retain(|entry| entry.trigger != trigger);
    entries.push(ActionEntry {
        trigger,
        action_id: action.id.as_u128(),
        payload_data: Some(action.payload.clone()),
    });
}

fn wrap_paint_in_layout(
    cx: &mut InternalLoweringCx<'_>,
    layout_node_id: WidgetId,
    paint_node_id: WidgetId,
    width: Option<f32>,
    height: Option<f32>,
    min_width: Option<f32>,
    max_width: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
    clip_to_bounds: bool,
    flex_grow: f32,
    flex_shrink: f32,
) -> WidgetId {
    let mut layout_builder = InternalIrBuilder::new(
        layout_node_id,
        Op::Layout(LayoutOp::Box {
            width,
            height,
            min_width,
            max_width,
            min_height,
            max_height,
            padding: [0.0; 4],
            flex_grow,
            flex_shrink,
            aspect_ratio: None,
        }),
    )
    .composite(CompositeStyle {
        clip_to_bounds,
        ..Default::default()
    });
    layout_builder.add_child(paint_node_id);
    layout_builder.build(cx)
}

fn resolve_line_height(font_size: f32, line_height: Option<f32>) -> f32 {
    line_height.unwrap_or(font_size * 1.2)
}

fn cap_max_height(
    max_height: Option<f32>,
    max_lines: Option<usize>,
    line_height: f32,
) -> Option<f32> {
    match max_lines {
        Some(lines) => {
            let line_cap = line_height * lines as f32;
            Some(max_height.map_or(line_cap, |existing| existing.min(line_cap)))
        }
        None => max_height,
    }
}

fn paragraph_line_height(line_height: f32, strut_line_height: Option<f32>) -> f32 {
    strut_line_height.map_or(line_height, |strut| line_height.max(strut))
}

fn paragraph_style_metadata(
    text_align: IrTextAlign,
    text_direction: IrTextDirection,
    text_width_basis: IrTextWidthBasis,
    max_lines: Option<usize>,
    overflow: IrTextOverflow,
    strut_line_height: Option<f32>,
    text_height_behavior: IrTextHeightBehavior,
) -> Option<IrTextParagraphStyle> {
    let style = IrTextParagraphStyle {
        text_align,
        text_direction,
        text_width_basis,
        max_lines,
        overflow,
        strut_line_height,
        text_height_behavior,
    };
    if style == IrTextParagraphStyle::default() {
        None
    } else {
        Some(style)
    }
}

fn should_clip_paragraph(max_lines: Option<usize>, overflow: IrTextOverflow) -> bool {
    max_lines.is_some() || overflow != IrTextOverflow::Visible
}

fn rich_text_line_height(
    runs: &[IrTextRun],
    fallback_size: f32,
    strut_line_height: Option<f32>,
) -> f32 {
    runs.iter()
        .map(|run| {
            if let Some(marker) = decode_inline_widget_marker(run.style.font_family.as_deref()) {
                marker.height
            } else {
                paragraph_line_height(
                    resolve_line_height(run.style.font_size, run.style.line_height),
                    strut_line_height,
                )
            }
        })
        .fold(
            paragraph_line_height(resolve_line_height(fallback_size, None), strut_line_height),
            f32::max,
        )
}

fn maybe_wrap_semantics(
    cx: &mut InternalLoweringCx<'_>,
    layout_node_id: WidgetId,
    semantics: Option<Semantics>,
    multiline: bool,
) -> WidgetId {
    if let Some(mut s) = semantics {
        if s.role == Role::Generic {
            s.role = Role::Text;
        }
        s.multiline = multiline;
        s.focusable |= s
            .actions
            .entries
            .iter()
            .any(|entry| entry.trigger == ActionTrigger::Default);
        let mut semantics_builder = InternalIrBuilder::new(cx.next_node_id(), Op::Semantics(s));
        semantics_builder.add_child(layout_node_id);
        semantics_builder.build(cx)
    } else {
        layout_node_id
    }
}

impl InternalLower for Text {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let layout_node_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let resolved_text = self.resolve_text(cx);
        let style = self.resolved_style(cx);
        let paragraph_style = paragraph_style_metadata(
            self.text_align,
            self.text_direction,
            self.text_width_basis,
            self.max_lines,
            self.overflow,
            self.strut_line_height,
            self.text_height_behavior,
        );
        let max_height = cap_max_height(
            self.max_height,
            self.max_lines,
            paragraph_line_height(
                resolve_line_height(style.font_size, style.line_height),
                self.strut_line_height,
            ),
        );
        let clip_to_bounds = should_clip_paragraph(self.max_lines, self.overflow);

        let paint_node_id = if self.needs_rich_text() {
            let runs = apply_selection_to_runs(
                vec![IrTextRun {
                    text: resolved_text,
                    style: style.clone(),
                }],
                self.selection_range,
                self.selection_color,
                self.selection_text_color,
            );
            InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRichText {
                    runs,
                    wrap: self.wrap,
                    caret_index: None,
                    caret_color: None,
                    caret_width: None,
                    caret_height: None,
                    caret_radius: None,
                    paragraph_style,
                }),
            )
            .build(cx)
        } else {
            InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText {
                    text: resolved_text,
                    size: style.font_size,
                    color: style.color,
                    underline: style.underline,
                    wrap: self.wrap,
                    caret_index: None,
                    caret_color: None,
                    caret_width: None,
                    caret_height: None,
                    caret_radius: None,
                    paragraph_style,
                }),
            )
            .build(cx)
        };

        let layout_node_id = wrap_paint_in_layout(
            cx,
            layout_node_id,
            paint_node_id,
            self.width,
            self.height,
            self.min_width,
            self.max_width,
            self.min_height,
            max_height,
            clip_to_bounds,
            self.flex_grow,
            self.flex_shrink,
        );

        maybe_wrap_semantics(cx, layout_node_id, self.semantics.clone(), false)
    }
}

impl InternalLower for RichText {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let layout_node_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let runs = self.lower_runs(cx);
        let runs = apply_selection_to_runs(
            runs,
            self.selection_range,
            self.selection_color,
            self.selection_text_color,
        );
        let paragraph_style = paragraph_style_metadata(
            self.text_align,
            self.text_direction,
            self.text_width_basis,
            self.max_lines,
            self.overflow,
            self.strut_line_height,
            self.text_height_behavior,
        );
        let max_height = cap_max_height(
            self.max_height,
            self.max_lines,
            rich_text_line_height(
                &runs,
                cx.env.theme.tokens.typography.body_medium_size,
                self.strut_line_height,
            ),
        );
        let clip_to_bounds = should_clip_paragraph(self.max_lines, self.overflow);
        let mut paint_builder = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                wrap: self.wrap,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style,
            }),
        );
        for inline_widget in &self.inline_widgets {
            let child_id = inline_widget.widget.lower(cx);
            paint_builder.add_child(child_id);
        }
        let paint_node_id = paint_builder.build(cx);
        if !self.annotations.is_empty() {
            cx.ir
                .custom_render_objects
                .insert(paint_node_id, Arc::new(self.annotations.clone()));
        }

        let layout_node_id = wrap_paint_in_layout(
            cx,
            layout_node_id,
            paint_node_id,
            self.width,
            self.height,
            self.min_width,
            self.max_width,
            self.min_height,
            max_height,
            clip_to_bounds,
            self.flex_grow,
            self.flex_shrink,
        );

        maybe_wrap_semantics(cx, layout_node_id, self.semantics.clone(), true)
    }
}
