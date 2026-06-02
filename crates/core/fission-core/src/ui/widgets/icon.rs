use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use fission_ir::{
    op::{Color, LayoutOp, Op, PaintOp, Stroke},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// The source of an [`Icon`]'s vector graphic.
///
/// - `Path` -- an SVG path data string (e.g. `"M12 2L2 22h20L12 2z"`).
/// - `File` -- a filesystem path to an SVG file.
/// - `SvgContent` -- inline SVG markup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IconSource {
    /// SVG path data string (`d` attribute content).
    Path(String),
    /// Filesystem path to an SVG file.
    File(String),
    /// Complete inline SVG markup.
    SvgContent(String),
}

impl Default for IconSource {
    fn default() -> Self {
        IconSource::Path(String::new())
    }
}

impl From<String> for IconSource {
    fn from(s: String) -> Self {
        IconSource::Path(s)
    }
}

/// A vector icon rendered from an SVG path, file, or inline SVG content.
///
/// Icons default to the theme's primary text colour and 24x24 layout points.
/// Use `color()`, `size()`, and `stroke()` to customise.
///
/// # Example
///
/// ```rust,ignore
/// // From an SVG path string
/// Icon::path("M12 2L2 22h20L12 2z")
///     .size(20.0)
///     .color(theme.tokens.colors.primary)
///
/// // From a file
/// Icon::file("assets/icons/star.svg").size(16.0)
///
/// // From inline SVG
/// Icon::svg("<svg>...</svg>")
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Icon {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// The vector graphic source.
    pub source: IconSource,
    /// Fill colour (falls back to the theme's primary text colour).
    pub color: Option<Color>,
    /// Layout size in points (default: 24.0).
    pub size: Option<f32>,
    /// Optional stroke (when set, the fill is suppressed).
    pub stroke: Option<Stroke>,
}

impl Icon {
    pub fn path(path: impl Into<String>) -> Self {
        Self {
            id: None,
            source: IconSource::Path(path.into()),
            color: None,
            size: None,
            stroke: None,
        }
    }

    pub fn file(path: impl Into<String>) -> Self {
        Self {
            id: None,
            source: IconSource::File(path.into()),
            color: None,
            size: None,
            stroke: None,
        }
    }

    pub fn svg(content: impl Into<String>) -> Self {
        Self {
            id: None,
            source: IconSource::SvgContent(content.into()),
            color: None,
            size: None,
            stroke: None,
        }
    }

    // Deprecated: new -> path
    pub fn new(path: impl Into<String>) -> Self {
        Self::path(path)
    }

    pub fn size(mut self, s: f32) -> Self {
        self.size = Some(s);
        self
    }

    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    pub fn stroke(mut self, s: Stroke) -> Self {
        self.stroke = Some(s);
        self
    }
}

impl InternalLower for Icon {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        let tokens = &cx.env.theme.tokens;
        let color = self.color.unwrap_or(tokens.colors.text_primary);
        let size = self.size.unwrap_or(24.0);

        // Paint Op
        let paint_op = match &self.source {
            IconSource::Path(d) => PaintOp::DrawPath {
                path: d.clone(),
                fill: if self.stroke.is_some() {
                    None
                } else {
                    Some(fission_ir::op::Fill::Solid(color))
                },
                stroke: self.stroke.clone(),
            },
            IconSource::File(f) => {
                let content = std::fs::read_to_string(f).unwrap_or_default();
                PaintOp::DrawSvg {
                    content,
                    fill: if self.stroke.is_some() {
                        None
                    } else {
                        Some(fission_ir::op::Fill::Solid(color))
                    },
                    stroke: self.stroke.clone(),
                }
            }
            IconSource::SvgContent(c) => PaintOp::DrawSvg {
                content: c.clone(),
                fill: if self.stroke.is_some() {
                    None
                } else {
                    Some(fission_ir::op::Fill::Solid(color))
                },
                stroke: self.stroke.clone(),
            },
        };

        let paint_id = InternalIrBuilder::new(cx.next_node_id(), Op::Paint(paint_op)).build(cx);

        let mut layout = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: Some(size),
                height: Some(size),
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
        layout.add_child(paint_id);
        layout.build(cx)
    }
}
