use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{Color as IrColor, LayoutOp, Op, PaintOp},
    NodeId, Semantics,
};
use serde::{Deserialize, Serialize};

/// The content source for a [`Text`] widget.
///
/// `Literal` renders a plain string. `Key` looks up a localised string in the
/// i18n registry at build time.
///
/// # Example
///
/// ```rust,ignore
/// // Literal text
/// Text::new("Hello, world!");
///
/// // i18n key (resolved via the active locale)
/// Text { content: TextContent::Key("greeting_label".into()), ..Default::default() }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextContent {
    /// A plain, inline string.
    Literal(String),
    /// An i18n key resolved at build time.
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

/// A read-only text label.
///
/// Renders a single run of styled text. Supports literal strings and i18n
/// keys, custom font size, colour, underline, and flex properties.
///
/// # Example
///
/// ```rust,ignore
/// Text::new("Total: 42")
///     .size(18.0)
///     .color(theme.tokens.colors.primary)
///     .underline(true)
///     .flex_grow(1.0)
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Text {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// The text content (literal string or i18n key).
    pub content: TextContent,
    /// Custom semantics for accessibility.
    pub semantics: Option<Semantics>,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Minimum width constraint.
    pub min_width: Option<f32>,
    /// Maximum width constraint.
    pub max_width: Option<f32>,
    /// Minimum height constraint.
    pub min_height: Option<f32>,
    /// Maximum height constraint.
    pub max_height: Option<f32>,
    /// Font size in points (falls back to the theme's body size).
    pub font_size: Option<f32>,
    /// Text colour (falls back to the theme's primary text colour).
    pub color: Option<IrColor>,
    /// Whether to render an underline decoration.
    pub underline: bool,
    /// Flex grow factor (0.0 by default -- does not stretch).
    pub flex_grow: f32,
    /// Flex shrink factor (0.0 by default -- does not shrink).
    pub flex_shrink: f32,
}

impl Text {
    pub fn new(content: impl Into<TextContent>) -> Self {
        Self {
            content: content.into(),
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

    // Stub for weight until we add font support to IR
    pub fn weight(self, _w: impl std::fmt::Debug) -> Self {
        self
    }

    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Text(self)
    }
}

impl Lower for Text {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_node_id = self.id.unwrap_or_else(|| cx.next_node_id());

        let resolved_text = match &self.content {
            TextContent::Literal(s) => s.clone(),
            TextContent::Key(key) => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("MISSING:{}", key)),
        };

        let paint_node_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawText {
                text: resolved_text,
                size: self
                    .font_size
                    .unwrap_or(cx.env.theme.tokens.typography.body_medium_size),
                color: self
                    .color
                    .unwrap_or(cx.env.theme.tokens.colors.text_primary),
                underline: self.underline,
                wrap: true,
                caret_index: None,
            }),
        )
        .build(cx);

        let mut layout_builder = NodeBuilder::new(
            layout_node_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: self.min_width,
                max_width: self.max_width,
                min_height: self.min_height,
                max_height: self.max_height,
                padding: [0.0; 4],
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
                aspect_ratio: None,
            }),
        );
        layout_builder.add_child(paint_node_id);
        let layout_node_id = layout_builder.build(cx);

        if let Some(mut s) = self.semantics.clone() {
            s.multiline = false;
            let mut semantics_builder = NodeBuilder::new(cx.next_node_id(), Op::Semantics(s));
            semantics_builder.add_child(layout_node_id);
            return semantics_builder.build(cx);
        }

        layout_node_id
    }
}
