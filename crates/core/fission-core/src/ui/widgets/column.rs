use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::Widget;
use fission_ir::op::{AlignItems, FlexWrap, JustifyContent};
use fission_ir::{FlexDirection, LayoutOp, Op, Semantics, WidgetId};
use serde::{Deserialize, Serialize};

/// A vertical flex container that lays out children in a column.
///
/// Children are arranged top-to-bottom. Use `align_items` to control
/// cross-axis (horizontal) alignment and `justify_content` for main-axis
/// (vertical) distribution.
///
/// # Example
///
/// ```rust,ignore
/// Column {
///     children: vec![
///         Text::new("Title").size(24.0).into(),
///         Text::new("Subtitle").size(14.0).into(),
///     ],
///     gap: Some(4.0),
///     align_items: AlignItems::Stretch,
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// The child widgets laid out top-to-bottom.
    pub children: Vec<Widget>,
    /// Custom semantics for accessibility.
    pub semantics: Option<Semantics>,
    /// Flex grow factor.
    pub flex_grow: f32,
    /// Flex shrink factor.
    pub flex_shrink: f32,
    /// Spacing between children in layout points.
    pub gap: Option<f32>,
    /// Whether children wrap when they overflow.
    pub wrap: FlexWrap,
    /// Cross-axis (horizontal) alignment (default: `Stretch`).
    pub align_items: AlignItems,
    /// Main-axis (vertical) distribution (default: `Start`).
    pub justify_content: JustifyContent,
}

impl Default for Column {
    fn default() -> Self {
        Self {
            id: None,
            children: Vec::new(),
            gap: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            semantics: None,
            wrap: FlexWrap::NoWrap,
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::Start,
        }
    }
}

impl Column {
    pub fn children(mut self, children: Vec<Widget>) -> Self {
        self.children = children;
        self
    }

    pub fn flex_grow(mut self, flex_grow: f32) -> Self {
        self.flex_grow = flex_grow;
        self
    }

    pub fn gap(mut self, gap: Option<f32>) -> Self {
        self.gap = gap;
        self
    }

    pub fn align_items(mut self, align: AlignItems) -> Self {
        self.align_items = align;
        self
    }

    pub fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.justify_content = justify;
        self
    }
}

impl InternalLower for Column {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let layout_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(layout_id);

        let mut builder = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                wrap: self.wrap,
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
                padding: [0.0; 4],
                gap: self.gap,
                align_items: self.align_items,
                justify_content: self.justify_content,
            }),
        );
        for child in &self.children {
            builder.add_child(child.lower(cx));
        }

        cx.pop_scope();

        let layout_id = builder.build(cx);

        if let Some(s) = &self.semantics {
            let mut semantics_builder =
                InternalIrBuilder::new(cx.next_node_id(), Op::Semantics(s.clone()));
            semantics_builder.add_child(layout_id);
            return semantics_builder.build(cx);
        }

        layout_id
    }
}
