use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{
        encode_text_paragraph_style, Color as IrColor, FontStyle as IrFontStyle, LayoutOp, Op,
        PaintOp, TextAlign as IrTextAlign, TextOverflow as IrTextOverflow,
        TextParagraphStyle as IrTextParagraphStyle, TextRun as IrTextRun,
    },
    CompositeStyle, NodeId, Semantics,
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextRunStyle {
    pub font_size: Option<f32>,
    pub color: Option<IrColor>,
    pub underline: bool,
    pub font_family: Option<String>,
    pub font_weight: Option<u16>,
    pub font_style: TextFontStyle,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub background_color: Option<IrColor>,
}

impl TextRunStyle {
    fn resolve(
        &self,
        theme: &fission_theme::Theme,
        fallback_size: Option<f32>,
        fallback_color: Option<IrColor>,
    ) -> fission_ir::op::TextStyle {
        fission_ir::op::TextStyle {
            font_size: self
                .font_size
                .or(fallback_size)
                .unwrap_or(theme.tokens.typography.body_medium_size),
            color: self
                .color
                .or(fallback_color)
                .unwrap_or(theme.tokens.colors.text_primary),
            underline: self.underline,
            font_family: self.font_family.clone(),
            font_weight: self.font_weight.unwrap_or(400),
            font_style: self.font_style.into(),
            line_height: self.line_height,
            letter_spacing: self.letter_spacing.unwrap_or(0.0),
            background_color: self.background_color,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichTextRun {
    pub text: String,
    pub style: TextRunStyle,
}

impl RichTextRun {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextRunStyle::default(),
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

    pub fn background_color(mut self, color: IrColor) -> Self {
        self.style.background_color = Some(color);
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
    pub font_weight: Option<u16>,
    pub font_style: Option<TextFontStyle>,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
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
            font_weight: self.font_weight.or(inherited.font_weight),
            font_style: self.font_style.unwrap_or(inherited.font_style),
            line_height: self.line_height.or(inherited.line_height),
            letter_spacing: self.letter_spacing.or(inherited.letter_spacing),
            background_color: self.background_color.or(inherited.background_color),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RichTextSpan {
    pub text: String,
    pub style: RichTextSpanStyle,
    pub children: Vec<RichTextSpan>,
    pub semantics_label: Option<String>,
}

pub type TextSpan = RichTextSpan;

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

    pub fn background_color(mut self, color: IrColor) -> Self {
        self.style.background_color = Some(color);
        self
    }

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics_label = Some(label.into());
        self
    }

    pub fn child<T>(mut self, child: T) -> Self
    where
        T: Into<RichTextSpan>,
    {
        self.children.push(child.into());
        self
    }

    pub fn children<I, T>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<RichTextSpan>,
    {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    fn push_runs(&self, inherited: &TextRunStyle, runs: &mut Vec<RichTextRun>) {
        let style = self.style.cascade(inherited);
        push_rich_text_run(runs, &self.text, &style);
        for child in &self.children {
            child.push_runs(&style, runs);
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
            has_override |= child.collect_semantics_text(out);
        }
        has_override
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
                font_weight: value.style.font_weight,
                font_style: Some(value.style.font_style),
                line_height: value.style.line_height,
                letter_spacing: value.style.letter_spacing,
                background_color: value.style.background_color,
            },
            children: Vec::new(),
            semantics_label: None,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Text {
    pub id: Option<NodeId>,
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
    pub wrap: bool,
    pub text_align: IrTextAlign,
    pub max_lines: Option<usize>,
    pub overflow: IrTextOverflow,
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

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn text_align(mut self, text_align: IrTextAlign) -> Self {
        self.text_align = text_align;
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

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics = Some(merge_semantics_label(self.semantics.take(), label));
        self
    }

    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Text(self)
    }

    fn resolve_text(&self, cx: &LoweringContext<'_>) -> String {
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

    fn resolved_style(&self, cx: &LoweringContext<'_>) -> fission_ir::op::TextStyle {
        fission_ir::op::TextStyle {
            font_size: self
                .font_size
                .unwrap_or(cx.env.theme.tokens.typography.body_medium_size),
            color: self
                .color
                .unwrap_or(cx.env.theme.tokens.colors.text_primary),
            underline: self.underline,
            font_family: self.font_family.clone(),
            font_weight: self.font_weight.unwrap_or(400),
            font_style: self.font_style.into(),
            line_height: self.line_height,
            letter_spacing: self.letter_spacing.unwrap_or(0.0),
            background_color: None,
        }
    }

    fn needs_rich_text(&self) -> bool {
        self.font_family.is_some()
            || self.font_weight.is_some()
            || self.font_style != TextFontStyle::Normal
            || self.line_height.is_some()
            || self.letter_spacing.unwrap_or(0.0) != 0.0
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RichText {
    pub id: Option<NodeId>,
    pub runs: Vec<RichTextRun>,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub wrap: bool,
    pub text_align: IrTextAlign,
    pub max_lines: Option<usize>,
    pub overflow: IrTextOverflow,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl RichText {
    pub fn new(runs: Vec<RichTextRun>) -> Self {
        Self {
            runs,
            wrap: true,
            ..Default::default()
        }
    }

    pub fn from_span<T>(span: T) -> Self
    where
        T: Into<RichTextSpan>,
    {
        Self::from_spans(std::iter::once(span))
    }

    pub fn from_spans<I, T>(spans: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<RichTextSpan>,
    {
        let spans: Vec<_> = spans.into_iter().map(Into::into).collect();
        let mut runs = Vec::new();
        let mut semantics_text = String::new();
        let mut has_semantics_override = false;

        for span in &spans {
            span.push_runs(&TextRunStyle::default(), &mut runs);
            has_semantics_override |= span.collect_semantics_text(&mut semantics_text);
        }

        let mut rich_text = Self::new(runs);
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

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines);
        self
    }

    pub fn overflow(mut self, overflow: IrTextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics = Some(merge_semantics_label(self.semantics.take(), label));
        self
    }

    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::RichText(self)
    }

    fn lower_runs(&self, cx: &LoweringContext<'_>) -> Vec<IrTextRun> {
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
    });
}

fn merge_semantics_label(semantics: Option<Semantics>, label: impl Into<String>) -> Semantics {
    let mut semantics = semantics.unwrap_or_default();
    semantics.label = Some(label.into());
    semantics
}

fn wrap_paint_in_layout(
    cx: &mut LoweringContext<'_>,
    layout_node_id: NodeId,
    paint_node_id: NodeId,
    width: Option<f32>,
    height: Option<f32>,
    min_width: Option<f32>,
    max_width: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
    clip_to_bounds: bool,
    flex_grow: f32,
    flex_shrink: f32,
) -> NodeId {
    let mut layout_builder = NodeBuilder::new(
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

fn paragraph_style_width(
    text_align: IrTextAlign,
    max_lines: Option<usize>,
    overflow: IrTextOverflow,
) -> Option<f32> {
    encode_text_paragraph_style(IrTextParagraphStyle {
        text_align,
        max_lines,
        overflow,
    })
}

fn should_clip_paragraph(max_lines: Option<usize>, overflow: IrTextOverflow) -> bool {
    max_lines.is_some() || overflow != IrTextOverflow::Visible
}

fn rich_text_line_height(runs: &[IrTextRun], fallback_size: f32) -> f32 {
    runs.iter()
        .map(|run| resolve_line_height(run.style.font_size, run.style.line_height))
        .fold(resolve_line_height(fallback_size, None), f32::max)
}

fn maybe_wrap_semantics(
    cx: &mut LoweringContext<'_>,
    layout_node_id: NodeId,
    semantics: Option<Semantics>,
    multiline: bool,
) -> NodeId {
    if let Some(mut s) = semantics {
        s.multiline = multiline;
        let mut semantics_builder = NodeBuilder::new(cx.next_node_id(), Op::Semantics(s));
        semantics_builder.add_child(layout_node_id);
        semantics_builder.build(cx)
    } else {
        layout_node_id
    }
}

impl Lower for Text {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_node_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let resolved_text = self.resolve_text(cx);
        let style = self.resolved_style(cx);
        let paragraph_width = paragraph_style_width(self.text_align, self.max_lines, self.overflow);
        let max_height = cap_max_height(
            self.max_height,
            self.max_lines,
            resolve_line_height(style.font_size, style.line_height),
        );
        let clip_to_bounds = should_clip_paragraph(self.max_lines, self.overflow);

        let paint_node_id = if self.needs_rich_text() {
            NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRichText {
                    runs: vec![IrTextRun {
                        text: resolved_text,
                        style: style.clone(),
                    }],
                    wrap: self.wrap,
                    caret_index: None,
                    caret_color: None,
                    // Static text piggybacks paragraph metadata onto the unused caret width slot.
                    caret_width: paragraph_width,
                }),
            )
            .build(cx)
        } else {
            NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText {
                    text: resolved_text,
                    size: style.font_size,
                    color: style.color,
                    underline: style.underline,
                    wrap: self.wrap,
                    caret_index: None,
                    caret_color: None,
                    // Static text piggybacks paragraph metadata onto the unused caret width slot.
                    caret_width: paragraph_width,
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

impl Lower for RichText {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_node_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let runs = self.lower_runs(cx);
        let paragraph_width = paragraph_style_width(self.text_align, self.max_lines, self.overflow);
        let max_height = cap_max_height(
            self.max_height,
            self.max_lines,
            rich_text_line_height(&runs, cx.env.theme.tokens.typography.body_medium_size),
        );
        let clip_to_bounds = should_clip_paragraph(self.max_lines, self.overflow);
        let paint_node_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRichText {
                runs,
                wrap: self.wrap,
                caret_index: None,
                caret_color: None,
                // Static text piggybacks paragraph metadata onto the unused caret width slot.
                caret_width: paragraph_width,
            }),
        )
        .build(cx);

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
