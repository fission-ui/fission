use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{
    op::{
        BackdropFilter, BoxAlignment, BoxGridPlacement, BoxPosition, BoxShadow, BoxStyle, Color,
        Fill, GridPlacement, LayoutOp, Length, Op, OrderedLayoutUnit, Overflow, PaintOp, Stroke,
    },
    CompositeStyle, WidgetId,
};
use serde::{Deserialize, Serialize};

use super::split_box_margin;

/// The universal wrapper widget: typed box layout, background fill, border,
/// padding, margin, overflow, aspect ratio, positioning, and shadow on one
/// child.
///
/// `Container` is the workhorse of layout composition. Use it whenever you
/// need to add visual decoration or spacing around a child widget.
///
/// # Example
///
/// ```rust,ignore
/// const CARD_MIN_WIDTH: f32 = 280.0;
/// const CARD_MAX_WIDTH: f32 = 520.0;
///
/// Container::new(Text::new("Card body"))
///     .bg(theme.tokens.colors.surface)
///     .border(theme.tokens.colors.border, 1.0)
///     .border_radius(theme.tokens.radii.large)
///     .padding_lengths(Length::all(Length::points(theme.tokens.spacing.l)))
///     .width_length(Length::clamp(
///         Length::points(CARD_MIN_WIDTH),
///         Length::percent(42.0),
///         Length::points(CARD_MAX_WIDTH),
///     ))
///     .flex_grow(1.0)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// The single child widget.
    pub child: Option<Widget>,

    // -- Layout constraints --
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
    /// Padding `[left, right, top, bottom]`.
    pub padding: [f32; 4],
    /// Flex grow factor (how much extra space this container absorbs).
    pub flex_grow: f32,
    /// Flex shrink factor (how much this container shrinks when space is tight).
    pub flex_shrink: f32,
    /// Declarative sizing, padding, aspect ratio, and overflow.
    #[serde(default)]
    pub box_style: BoxStyle,
    /// Outer spacing `[left, right, top, bottom]`.
    #[serde(default)]
    pub margin: [f32; 4],

    // -- Visual style --
    /// Background fill.
    pub background_fill: Option<Fill>,
    /// Legacy background fill colour.
    pub background_color: Option<Color>,
    /// Border stroke colour.
    pub border_color: Option<Color>,
    /// Border stroke width in layout points.
    pub border_width: f32,
    /// Corner radius for rounded corners.
    pub border_radius: f32,
    /// Optional drop shadow.
    pub shadow: Option<BoxShadow>,
    /// Additional shadows drawn behind the container in order.
    pub shadows: Vec<BoxShadow>,
    /// Filter applied to content painted behind this container.
    pub backdrop_filter: Option<BackdropFilter>,
}

impl Default for Container {
    fn default() -> Self {
        Self {
            id: None,
            child: None,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            box_style: BoxStyle::default(),
            margin: [0.0; 4],
            background_fill: None,
            background_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            shadow: None,
            shadows: Vec::new(),
            backdrop_filter: None,
        }
    }
}
impl Container {
    pub fn new(child: impl Into<Widget>) -> Self {
        let mut container = Self {
            child: Some(child.into()),
            ..Default::default()
        };
        container.box_style.alignment = BoxAlignment::Stretch;
        container
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Some(w);
        self.height = Some(h);
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// Sets a typed preferred width.
    pub fn width_length(mut self, width: Length) -> Self {
        self.box_style.width = Some(width);
        self
    }

    /// Sets a typed preferred height.
    pub fn height_length(mut self, height: Length) -> Self {
        self.box_style.height = Some(height);
        self
    }

    /// Sets a typed minimum width.
    pub fn min_width_length(mut self, width: Length) -> Self {
        self.box_style.min_width = Some(width);
        self
    }

    /// Sets a typed maximum width.
    pub fn max_width_length(mut self, width: Length) -> Self {
        self.box_style.max_width = Some(width);
        self
    }

    /// Sets a typed minimum height.
    pub fn min_height_length(mut self, height: Length) -> Self {
        self.box_style.min_height = Some(height);
        self
    }

    /// Sets a typed maximum height.
    pub fn max_height_length(mut self, height: Length) -> Self {
        self.box_style.max_height = Some(height);
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

    pub fn padding_all(mut self, p: f32) -> Self {
        self.padding = [p; 4];
        self
    }

    pub fn padding(mut self, padding: [f32; 4]) -> Self {
        self.padding = padding;
        self
    }

    /// Sets typed `[left, right, top, bottom]` padding.
    pub fn padding_lengths(mut self, padding: [Length; 4]) -> Self {
        self.box_style.padding = Some(padding);
        self
    }

    /// Sets equal point-based margin on every edge.
    pub fn margin_all(mut self, margin: f32) -> Self {
        self.margin = [margin; 4];
        self
    }

    /// Sets point-based `[left, right, top, bottom]` margin.
    pub fn margin(mut self, margin: [f32; 4]) -> Self {
        self.margin = margin;
        self
    }

    /// Sets typed `[left, right, top, bottom]` margin.
    pub fn margin_lengths(mut self, margin: [Length; 4]) -> Self {
        self.box_style.margin = Some(margin);
        self
    }

    /// Aligns the child inside this container.
    pub fn align_child(mut self, alignment: BoxAlignment) -> Self {
        self.box_style.alignment = alignment;
        self
    }

    /// Sets a non-negative width-to-height ratio.
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.box_style.aspect_ratio = Some(OrderedLayoutUnit(ratio.max(0.0)));
        self
    }

    /// Absolutely positions this container with point-based offsets.
    pub fn positioned(
        mut self,
        left: Option<f32>,
        top: Option<f32>,
        right: Option<f32>,
        bottom: Option<f32>,
    ) -> Self {
        self.box_style.position = Some(BoxPosition {
            left: left.map(Length::Points),
            top: top.map(Length::Points),
            right: right.map(Length::Points),
            bottom: bottom.map(Length::Points),
        });
        self
    }

    /// Positions this box with typed offsets relative to its positioned ancestor.
    pub fn positioned_lengths(
        mut self,
        left: Option<Length>,
        top: Option<Length>,
        right: Option<Length>,
        bottom: Option<Length>,
    ) -> Self {
        self.box_style.position = Some(BoxPosition {
            left,
            top,
            right,
            bottom,
        });
        self
    }

    /// Places this container at a one-based grid row and column.
    pub fn grid_cell(mut self, row: i16, column: i16) -> Self {
        self.box_style.grid = Some(BoxGridPlacement {
            row_start: GridPlacement::Line(row),
            col_start: GridPlacement::Line(column),
            ..Default::default()
        });
        self
    }

    /// Spans this container across parent grid rows and columns.
    pub fn grid_span(mut self, rows: u16, columns: u16) -> Self {
        let placement = self.box_style.grid.get_or_insert_default();
        placement.row_end = GridPlacement::Span(rows.max(1));
        placement.col_end = GridPlacement::Span(columns.max(1));
        self
    }

    /// Clips painting and descendants to this container's bounds.
    pub fn clip_overflow(mut self, clip: bool) -> Self {
        self.box_style.overflow = if clip {
            Overflow::Clip
        } else {
            Overflow::Visible
        };
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow;
        self.box_style.flex_grow = Some(OrderedLayoutUnit(grow));
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = shrink;
        self.box_style.flex_shrink = Some(OrderedLayoutUnit(shrink));
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.background_fill = Some(Fill::Solid(color));
        self.background_color = Some(color);
        self
    }

    pub fn bg_fill(mut self, fill: Fill) -> Self {
        self.background_fill = Some(fill);
        self.background_color = None;
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }

    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    pub fn shadow(mut self, shadow: BoxShadow) -> Self {
        self.shadow = Some(shadow);
        self
    }

    pub fn shadows(mut self, shadows: Vec<BoxShadow>) -> Self {
        self.shadows = shadows;
        self
    }

    /// Blurs content behind this container, clipped to its rounded bounds.
    pub fn backdrop_blur(mut self, sigma: f32) -> Self {
        self.backdrop_filter = Some(BackdropFilter::Blur(sigma.max(0.0)));
        self
    }
}

impl InternalLower for Container {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let mut children_ids = Vec::new();

        if let Some(filter) = self.backdrop_filter {
            let paint = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::BackdropFilter {
                    filter,
                    corner_radius: self.border_radius,
                }),
            )
            .build(cx);
            children_ids.push(paint);
        }

        // 1. Background Layer (PaintOp -> AbsoluteFill)
        if self.background_fill.is_some()
            || self.background_color.is_some()
            || self.border_color.is_some()
            || self.shadow.is_some()
            || !self.shadows.is_empty()
        {
            for shadow in &self.shadows {
                let paint = InternalIrBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: None,
                        stroke: None,
                        corner_radius: self.border_radius,
                        shadow: Some(*shadow),
                    }),
                )
                .build(cx);
                children_ids.push(paint);
            }
            let paint = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: self
                        .background_fill
                        .clone()
                        .or_else(|| self.background_color.map(Fill::Solid)),
                    stroke: self.border_color.map(|c| Stroke {
                        fill: Fill::Solid(c),
                        width: self.border_width,
                        dash_array: None,
                        line_cap: fission_ir::op::LineCap::Butt,
                        line_join: fission_ir::op::LineJoin::Miter,
                    }),
                    corner_radius: self.border_radius,
                    shadow: self.shadow,
                }),
            )
            .build(cx);
            children_ids.push(paint);
        }

        // 2. Content Layer
        if let Some(child) = &self.child {
            children_ids.push(child.lower(cx));
        }

        cx.pop_scope();

        let mut style = self.box_style.clone();
        style.width = style.width.or(self.width.map(Length::Points));
        style.height = style.height.or(self.height.map(Length::Points));
        style.min_width = style.min_width.or(self.min_width.map(Length::Points));
        style.max_width = style.max_width.or(self.max_width.map(Length::Points));
        style.min_height = style.min_height.or(self.min_height.map(Length::Points));
        style.max_height = style.max_height.or(self.max_height.map(Length::Points));
        style.padding = style
            .padding
            .or_else(|| (self.padding != [0.0; 4]).then(|| self.padding.map(Length::Points)));
        style.margin = style
            .margin
            .or_else(|| (self.margin != [0.0; 4]).then(|| self.margin.map(Length::Points)));
        let margin_style = split_box_margin(&mut style);
        let position = style.position.take();
        let grid = style.grid.take();
        let flex_grow = style
            .flex_grow
            .map(|value| value.0)
            .unwrap_or(self.flex_grow);
        let flex_shrink = style
            .flex_shrink
            .map(|value| value.0)
            .unwrap_or(self.flex_shrink);

        let mut layout = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::StyledBox {
                style: style.clone(),
                flex_grow,
                flex_shrink,
            }),
        )
        .composite(CompositeStyle {
            clip_to_bounds: style.overflow == Overflow::Clip,
            ..Default::default()
        });

        for cid in children_ids {
            layout.add_child(cid);
        }

        let mut result = layout.build(cx);
        if let Some(margin_style) = margin_style {
            let mut outer = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::StyledBox {
                    style: margin_style,
                    flex_grow,
                    flex_shrink,
                }),
            );
            outer.add_child(result);
            result = outer.build(cx);
        }
        if let Some(position) = position {
            let mut outer = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::PositionedLengths {
                    left: position.left,
                    top: position.top,
                    right: position.right,
                    bottom: position.bottom,
                    width: None,
                    height: None,
                }),
            );
            outer.add_child(result);
            result = outer.build(cx);
        }
        if let Some(grid) = grid {
            let mut outer = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::GridItem {
                    row_start: grid.row_start,
                    row_end: grid.row_end,
                    col_start: grid.col_start,
                    col_end: grid.col_end,
                }),
            );
            outer.add_child(result);
            result = outer.build(cx);
        }
        result
    }
}
