use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use fission_ir::{
    op::{EmbedKind, LayoutOp, Op},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// A platform-native map widget backed by MapKit (`MKMapView`) on Apple
/// platforms.
///
/// The map is rendered by the platform's native map view and embedded into the
/// Fission layout as an opaque surface. Use
/// [`crate::internal::BuildCtx::map_controls`] to create action envelopes for
/// programmatic camera control.
///
/// # Example
///
/// ```rust,ignore
/// Map {
///     center: (37.7749, -122.4194), // San Francisco
///     zoom: 12.0,
///     width: Some(400.0),
///     height: Some(300.0),
///     ..Default::default()
/// }
/// .into();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Map {
    /// Stable widget identity (auto-derived from center coordinates if `None`).
    pub id: Option<WidgetId>,
    /// Center coordinate as `(latitude, longitude)`.
    pub center: (f64, f64),
    /// Zoom level (0 = world, ~20 = building-level).
    pub zoom: f32,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Whether to show the user's current location on the map.
    pub show_user_location: bool,
    /// Whether the user can pan and zoom the map interactively.
    pub interactive: bool,
}

impl Default for Map {
    fn default() -> Self {
        Self {
            id: None,
            center: (0.0, 0.0),
            zoom: 10.0,
            width: None,
            height: None,
            show_user_location: false,
            interactive: true,
        }
    }
}

impl Map {}

impl InternalLower for Map {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let widget_id = self.id.unwrap_or_else(|| {
            WidgetId::explicit(&format!("map:{}:{}", self.center.0, self.center.1))
        });
        let layout_id = cx.widget_node_id(widget_id);

        let embed_id = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Map,
                widget_id,
                width: self.width,
                height: self.height,
            }),
        )
        .build(cx);

        let mut layout_builder = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
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
        layout_builder.add_child(embed_id);
        layout_builder.build(cx)
    }
}
