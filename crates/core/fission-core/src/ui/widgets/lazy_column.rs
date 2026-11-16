use crate::authoring::Lower;
use crate::lowering::{IrBuilder, LoweringContext};
use crate::ui::Widget;
use fission_ir::{
    op::{FlexDirection, LayoutOp, Op},
    WidgetId,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

type ItemBuilder = Arc<dyn Fn(usize) -> Widget + Send + Sync + 'static>;
type ItemKey = Arc<dyn Fn(usize) -> WidgetId + Send + Sync + 'static>;

/// A virtualized vertical list backed by widgets that are already constructed.
///
/// This type remains useful for small or serialized lists. For large data sets,
/// use [`LazyColumn::with_item_builder`] so invisible rows are not constructed.
/// Each item is assumed to have a uniform `item_height`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LazyColumn {
    /// Explicit node identity (used for scroll-offset tracking).
    pub id: Option<WidgetId>,
    /// All items in the list (only the visible slice is lowered each frame).
    pub children: Vec<Widget>,
    /// Uniform height of each item in layout points. Set to 0 to disable
    /// virtualisation.
    pub item_height: f32,
}

/// A virtualized vertical list that creates rows only as they become visible.
///
/// Construct it with [`LazyColumn::with_item_builder`] or
/// [`LazyColumn::with_keyed_item_builder`]. The builder callback is runtime
/// data and is omitted from serialization; a deserialized builder therefore
/// renders as an empty list.
#[derive(Clone, Serialize, Deserialize)]
pub struct LazyColumnBuilder {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<WidgetId>,
    pub(crate) item_height: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_width: Option<f32>,
    #[serde(default)]
    item_count: usize,
    #[serde(skip)]
    item_builder: Option<ItemBuilder>,
    #[serde(skip)]
    item_key: Option<ItemKey>,
    #[serde(skip)]
    visible_range: Range<usize>,
    #[serde(skip)]
    pub(crate) visible_rows: Vec<Widget>,
}

impl fmt::Debug for LazyColumnBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyColumnBuilder")
            .field("id", &self.id)
            .field("item_height", &self.item_height)
            .field("content_width", &self.content_width)
            .field("item_count", &self.item_count)
            .field("has_item_builder", &self.item_builder.is_some())
            .finish()
    }
}

impl LazyColumn {
    /// Creates a virtualized list without first constructing every row.
    ///
    /// The item's position is used as its identity. If rows can be inserted,
    /// removed, or reordered, use [`Self::with_keyed_item_builder`] so state
    /// remains attached to the same data item.
    pub fn with_item_builder(
        item_count: usize,
        item_height: f32,
        builder: impl Fn(usize) -> Widget + Send + Sync + 'static,
    ) -> LazyColumnBuilder {
        Self::with_keyed_item_builder(
            item_count,
            item_height,
            |index| WidgetId::derived(0, &[index as u32]),
            builder,
        )
    }

    /// Creates a virtualized list using a durable key for each row.
    ///
    /// The builder is called only for visible rows and the one-row overscan
    /// buffer. `key` runs before `builder`, so the ordinary component build
    /// scope, retained local state and reducers all use the data item's identity.
    /// Return the same key for the same data item even when its position changes.
    pub fn with_keyed_item_builder(
        item_count: usize,
        item_height: f32,
        key: impl Fn(usize) -> WidgetId + Send + Sync + 'static,
        builder: impl Fn(usize) -> Widget + Send + Sync + 'static,
    ) -> LazyColumnBuilder {
        LazyColumnBuilder {
            item_height,
            item_count,
            item_builder: Some(Arc::new(builder)),
            item_key: Some(Arc::new(key)),
            ..LazyColumnBuilder::default()
        }
    }
}

impl LazyColumnBuilder {
    pub(crate) fn prepare(mut self) -> Self {
        let Some(builder) = self.item_builder.take() else {
            return self;
        };
        let key = self
            .item_key
            .take()
            .expect("an item builder has a key selector");
        self.id = self
            .id
            .or_else(crate::build::current_widget_id)
            .or_else(|| crate::build::next_implicit_widget_id(0xF155_1A27));
        let view =
            crate::build::try_current_runtime_state().map(|_| crate::build::current::<()>().1);
        let list_id = self.id.unwrap_or_else(WidgetId::app_root);
        let height = view.as_ref().map_or(600.0, |view| {
            if view.layout().is_none() && view.viewport_size().height > 0.0 {
                view.viewport_size().height
            } else {
                viewport_height(view.layout(), list_id)
            }
        });
        let offset = view
            .map(|view| view.runtime().scroll.get_offset(list_id))
            .unwrap_or_default();
        self.visible_range = visible_range(self.item_count, self.item_height, height, offset);
        self.visible_rows = self
            .visible_range
            .clone()
            .map(|index| {
                let row_id = item_scope(list_id, key(index));
                crate::build::with_widget_id(row_id, || {
                    // This is the ordinary authoring pass: state and resource/reducer
                    // registration happen before the shell closes the build scope.
                    let widget = builder(index);
                    crate::internal::resolve_widget_identities_owned(widget, row_id)
                })
            })
            .collect();
        self
    }

    /// Assigns an explicit identity used to retain this list's scroll offset.
    pub fn id(mut self, id: impl Into<WidgetId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets a known width for the list's content while leaving the viewport
    /// width to its parent layout constraints.
    pub fn content_width(mut self, width: f32) -> Self {
        self.content_width = Some(width.max(0.0));
        self
    }
}

impl Default for LazyColumnBuilder {
    fn default() -> Self {
        Self {
            id: None,
            item_height: 0.0,
            content_width: None,
            item_count: 0,
            item_builder: None,
            item_key: None,
            visible_range: 0..0,
            visible_rows: Vec::new(),
        }
    }
}

impl Lower for LazyColumn {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let scroll_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        lower_lazy_column(
            cx,
            scroll_id,
            self.children.len(),
            self.item_height,
            None,
            None,
            |index, column_id, cx| {
                let child_scope = WidgetId::derived(column_id.as_u128(), &[index as u32]);
                cx.with_scope(child_scope, |cx| self.children[index].lower(cx))
            },
        )
    }
}

impl Lower for LazyColumnBuilder {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let scroll_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        lower_lazy_column(
            cx,
            scroll_id,
            if self.visible_rows.is_empty() {
                0
            } else {
                self.item_count
            },
            self.item_height,
            self.content_width,
            Some(self.visible_range.clone()),
            |index, _column_id, cx| {
                let row = &self.visible_rows[index - self.visible_range.start];
                let row_id = row
                    .declared_id()
                    .expect("prepared lazy rows have resolved identities");
                // A row's internal paint/layout nodes must be keyed too, not
                // allocated from the surrounding list's positional sequence.
                cx.with_scope(row_id, |cx| row.lower(cx))
            },
        )
    }
}

fn lower_lazy_column(
    cx: &mut LoweringContext,
    scroll_id: WidgetId,
    total_count: usize,
    item_height: f32,
    content_width: Option<f32>,
    prepared_range: Option<Range<usize>>,
    mut lower_item: impl FnMut(usize, WidgetId, &mut LoweringContext) -> WidgetId,
) -> WidgetId {
    let virtualized = item_height > 0.0;

    // Use the previous layout to estimate the viewport before lowering this
    // frame. The first frame has no previous geometry, so use a conservative
    // fallback rather than constructing the entire data set.
    let range = prepared_range.unwrap_or_else(|| {
        visible_range(
            total_count,
            item_height,
            viewport_height(cx.layout, scroll_id),
            cx.runtime_state.scroll.get_offset(scroll_id),
        )
    });
    let (start_index, end_index) = (range.start, range.end);
    let (top_spacer_height, bottom_spacer_height) = if virtualized {
        (
            item_height.max(1.0) * start_index as f32,
            item_height.max(1.0) * total_count.saturating_sub(end_index) as f32,
        )
    } else {
        (0.0, 0.0)
    };

    let col_id = cx.next_node_id();
    let mut column_children = Vec::new();

    if top_spacer_height > 0.0 {
        let spacer_id = WidgetId::derived(col_id.as_u128(), &[u32::MAX - 1]);
        column_children.push(
            IrBuilder::new(
                spacer_id,
                Op::Layout(LayoutOp::Box {
                    width: None,
                    height: Some(top_spacer_height),
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                }),
            )
            .build(cx),
        );
    }

    for index in start_index..end_index {
        column_children.push(lower_item(index, col_id, cx));
    }

    if bottom_spacer_height > 0.0 {
        let spacer_id = WidgetId::derived(col_id.as_u128(), &[u32::MAX]);
        column_children.push(
            IrBuilder::new(
                spacer_id,
                Op::Layout(LayoutOp::Box {
                    width: None,
                    height: Some(bottom_spacer_height),
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                }),
            )
            .build(cx),
        );
    }

    let mut col = IrBuilder::new(
        col_id,
        Op::Layout(LayoutOp::Flex {
            direction: FlexDirection::Column,
            wrap: fission_ir::op::FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 0.0,
            padding: [0.0; 4],
            gap: None,
            line_gap: None,
            align_items: fission_ir::op::AlignItems::Stretch,
            justify_content: fission_ir::op::JustifyContent::Start,
        }),
    );
    col.add_children(column_children);
    let col_id = col.build(cx);

    let content_id = cx.next_node_id();
    let mut content_box = IrBuilder::new(
        content_id,
        Op::Layout(LayoutOp::Box {
            width: content_width,
            height: None,
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
    content_box.add_child(col_id);
    let content_id = content_box.build(cx);

    let mut scroll = IrBuilder::new(
        scroll_id,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            // Let parent constraints define the viewport. Clamping this to the
            // app window width clips rows in an enclosing horizontal scroller.
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 1.0,
            flex_shrink: 1.0,
        }),
    );
    scroll.add_child(content_id);
    if virtualized {
        cx.mark_scroll_build_dependency(scroll_id);
    }
    scroll.build(cx)
}

fn item_scope(column_id: WidgetId, key: WidgetId) -> WidgetId {
    let key = key.as_u128();
    WidgetId::derived(
        column_id.as_u128(),
        &[
            (key >> 96) as u32,
            (key >> 64) as u32,
            (key >> 32) as u32,
            key as u32,
        ],
    )
}

fn viewport_height(layout: Option<&crate::LayoutSnapshot>, scroll_id: WidgetId) -> f32 {
    let Some(layout) = layout else {
        return 600.0;
    };
    let viewport = layout.viewport_size.height;
    let height = layout
        .get_node_geometry(scroll_id)
        .map(|geometry| geometry.rect.height())
        .filter(|height| *height > 0.0)
        .unwrap_or(viewport);
    let height = if viewport > 0.0 {
        height.min(viewport)
    } else {
        height
    };
    if height > 0.0 {
        height
    } else {
        600.0
    }
}

fn visible_range(count: usize, item_height: f32, viewport: f32, offset: f32) -> Range<usize> {
    if item_height <= 0.0 {
        return 0..count;
    }
    let height = item_height.max(1.0);
    let max_offset = (count as f32 * height - viewport.max(0.0)).max(0.0);
    let offset = offset.clamp(0.0, max_offset);
    let start = ((offset / height).floor() as usize).min(count);
    let visible = (viewport / height).ceil() as usize + 1;
    start..start.saturating_add(visible).min(count)
}
