use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use fission_ir::{
    op::{LayoutOp, Op},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// An invisible widget that occupies space.
///
/// Use `Spacer` to push siblings apart in a [`Row`](super::Row) or
/// [`Column`](super::Column). With `flex_grow > 0`, a spacer absorbs all
/// remaining space. With fixed `width` / `height`, it acts as a rigid gap.
///
/// # Example
///
/// ```rust,ignore
/// Row {
///     children: vec![
///         Text::new("Left").into(),
///         Spacer { flex_grow: 1.0, ..Default::default() }.into(),
///         Text::new("Right").into(),
///     ],
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Spacer {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Flex grow factor (set > 0 to absorb remaining space).
    pub flex_grow: f32,
}

impl InternalLower for Spacer {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: self.flex_grow,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        )
        .build(cx)
    }
}
