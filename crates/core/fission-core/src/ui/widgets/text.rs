use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{Color as IrColor, FontStyle as IrFontStyle, LayoutOp, Op, PaintOp, TextRun as IrTextRun},
    NodeId, Semantics,
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
            color: self.color.unwrap_or(cx.env.theme.tokens.colors.text_primary),
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
    );
    layout_builder.add_child(paint_node_id);
    layout_builder.build(cx)
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

        let paint_node_id = if self.needs_rich_text() {
            NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRichText {
                    runs: vec![IrTextRun {
                        text: resolved_text,
                        style: self.resolved_style(cx),
                    }],
                    wrap: self.wrap,
                    caret_index: None,
                }),
            )
            .build(cx)
        } else {
            let style = self.resolved_style(cx);
            NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText {
                    text: resolved_text,
                    size: style.font_size,
                    color: style.color,
                    underline: style.underline,
                    wrap: self.wrap,
                    caret_index: None,
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
            self.max_height,
            self.flex_grow,
            self.flex_shrink,
        );

        maybe_wrap_semantics(cx, layout_node_id, self.semantics.clone(), false)
    }
}

impl Lower for RichText {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_node_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let paint_node_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRichText {
                runs: self.lower_runs(cx),
                wrap: self.wrap,
                caret_index: None,
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
            self.max_height,
            self.flex_grow,
            self.flex_shrink,
        );

        maybe_wrap_semantics(cx, layout_node_id, self.semantics.clone(), true)
    }
}
