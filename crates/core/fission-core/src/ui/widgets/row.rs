use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::Widget;
use fission_ir::op::{AlignItems, FlexWrap, JustifyContent};
use fission_ir::{FlexDirection, LayoutOp, Op, Semantics, WidgetId};
use serde::{Deserialize, Serialize};

/// A horizontal flex container that lays out children in a row.
///
/// Children are arranged left-to-right (in LTR locales). Use `align_items` to
/// control cross-axis (vertical) alignment and `justify_content` for main-axis
/// (horizontal) distribution.
///
/// # Example
///
/// ```rust,ignore
/// Row {
///     children: vec![
///         Icon::path("M12 2L2 22h20L12 2z").into(),
///         Text::new("Warning").into(),
///     ],
///     gap: Some(8.0),
///     align_items: AlignItems::Center,
///     justify_content: JustifyContent::Start,
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// The child widgets laid out left-to-right.
    pub children: Vec<Widget>,
    /// Custom semantics for accessibility.
    pub semantics: Option<Semantics>,
    /// Flex grow factor.
    pub flex_grow: f32,
    /// Flex shrink factor.
    pub flex_shrink: f32,
    /// Spacing between children in layout points.
    pub gap: Option<f32>,
    /// Whether children wrap to a new line when they overflow.
    pub wrap: FlexWrap,
    /// Cross-axis (vertical) alignment of children (default: `Center`).
    pub align_items: AlignItems,
    /// Main-axis (horizontal) distribution of children (default: `Start`).
    pub justify_content: JustifyContent,
}

impl Default for Row {
    fn default() -> Self {
        Self {
            id: None,
            children: Vec::new(),
            semantics: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            gap: None,
            wrap: FlexWrap::NoWrap,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
        }
    }
}

impl Row {
    pub fn children(mut self, children: Vec<Widget>) -> Self {
        self.children = children;
        self
    }

    pub fn flex_grow(mut self, flex_grow: f32) -> Self {
        self.flex_grow = flex_grow;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap);
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

impl InternalLower for Row {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let layout_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(layout_id);

        let mut builder = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
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
