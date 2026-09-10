use crate::stack::VStack;
use fission_core::ui::{Container, Positioned, Row, Spacer, Text, Widget, ZStack};
use fission_core::WidgetId;
use fission_ir::op::AlignItems;
use fission_ir::op::Color;
use serde::{Deserialize, Serialize};

const IMPLICIT_TIMELINE_ID_SALT: u32 = 0x5449_4d45;
const LEGACY_ENTRY_ID_PATH: u32 = 0x454e_5452;

/// One event displayed in a [`Timeline`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineItem {
    /// Primary event label.
    pub title: String,
    /// Optional supporting detail.
    pub description: Option<String>,
    /// Optional preformatted time or date label.
    pub timestamp: Option<String>,
}

/// Ordered vertical presentation of related events.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timeline {
    /// Events displayed from first to last.
    pub items: Vec<TimelineItem>,
}

/// One retained, interactive entry in a [`TimelineLayout`].
///
/// Use this anatomy when a timeline item is richer than a title and supporting
/// text. The timeline continues to own marker alignment and connector
/// continuity while the application supplies semantic retained content.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineEntry {
    /// Stable identity for this entry. Derive it from the represented data.
    pub id: WidgetId,
    /// Retained item content, such as a card or interactive workflow step.
    pub child: Widget,
    /// Optional retained marker. The design-system dot is used when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<Widget>,
}

impl TimelineEntry {
    /// Creates a retained timeline entry using the design-system marker.
    pub fn new(id: WidgetId, child: impl Into<Widget>) -> Self {
        Self {
            id,
            child: child.into(),
            marker: None,
        }
    }

    /// Replaces the design-system marker with retained custom presentation.
    pub fn marker(mut self, marker: impl Into<Widget>) -> Self {
        self.marker = Some(marker.into());
        self
    }
}

/// Rich timeline anatomy for variable-height retained entries.
///
/// Entries may contain cards, controls, and other interactive content. The
/// layout draws one continuous connector behind their markers, including the
/// configured inter-entry gap, without requiring callers to reconstruct the
/// rail with ad-hoc stacks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineLayout {
    /// Optional stable identity for the complete timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<WidgetId>,
    /// Ordered retained entries.
    pub entries: Vec<TimelineEntry>,
    /// Width reserved for the marker rail.
    pub marker_width: f32,
    /// Marker height used to terminate the first and last connector segments.
    pub marker_extent: f32,
    /// Space between the marker rail and retained content.
    pub content_gap: f32,
    /// Vertical space between entries. The connector crosses this space.
    pub item_gap: f32,
    /// Optional connector width override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_width: Option<f32>,
    /// Optional connector colour override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_color: Option<Color>,
}

impl TimelineLayout {
    /// Creates rich timeline anatomy with compact design-system defaults.
    pub fn new(entries: Vec<TimelineEntry>) -> Self {
        Self {
            id: None,
            entries,
            marker_width: 20.0,
            marker_extent: 12.0,
            content_gap: 8.0,
            item_gap: 16.0,
            connector_width: None,
            connector_color: None,
        }
    }

    /// Uses a stable identity for the complete timeline.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Configures the marker rail and marker extent.
    pub fn marker_geometry(mut self, width: f32, extent: f32) -> Self {
        self.marker_width = width;
        self.marker_extent = extent;
        self
    }

    /// Configures horizontal content spacing and vertical item spacing.
    pub fn spacing(mut self, content_gap: f32, item_gap: f32) -> Self {
        self.content_gap = content_gap;
        self.item_gap = item_gap;
        self
    }

    /// Overrides the connector paint while preserving timeline ownership.
    pub fn connector(mut self, width: f32, color: Color) -> Self {
        self.connector_width = Some(width);
        self.connector_color = Some(color);
        self
    }
}

impl From<Timeline> for Widget {
    fn from(component: Timeline) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.timeline;
        let tokens = &view.env().theme.tokens;
        let timeline_id = fission_core::build::next_implicit_widget_id(IMPLICIT_TIMELINE_ID_SALT)
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.timeline"));
        let entries = component
            .items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let mut content_children = vec![Text::new(item.title.clone())
                    .size(tokens.typography.body_large_size)
                    .color(tokens.colors.text_primary)
                    .into()];

                if let Some(ts) = &item.timestamp {
                    content_children.push(
                        Text::new(ts.clone())
                            .size(12.0)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    );
                }

                if let Some(desc) = &item.description {
                    content_children.push(
                        Text::new(desc.clone())
                            .color(tokens.colors.text_secondary)
                            .into(),
                    );
                }

                TimelineEntry::new(
                    WidgetId::derived(timeline_id.as_u128(), &[LEGACY_ENTRY_ID_PATH, index as u32]),
                    VStack {
                        spacing: Some(4.0),
                        children: content_children,
                    },
                )
            })
            .collect();

        let mut layout = TimelineLayout::new(entries).id(timeline_id);
        layout.marker_extent = theme.dot_size;
        layout.connector_width = Some(theme.line_width);
        layout.connector_color = Some(theme.line_color);
        layout.into()
    }
}

impl From<TimelineLayout> for Widget {
    fn from(component: TimelineLayout) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.timeline;
        let marker_width = component.marker_width.max(component.marker_extent);
        let marker_extent = component.marker_extent.max(0.0);
        let connector_width = component
            .connector_width
            .unwrap_or(theme.line_width)
            .max(0.0);
        let connector_color = component.connector_color.unwrap_or(theme.line_color);
        let entry_count = component.entries.len();
        let children = component
            .entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                let first = index == 0;
                let last = index + 1 == entry_count;
                let connector = (!first || !last).then(|| Positioned {
                    left: Some((marker_width - connector_width) / 2.0),
                    top: Some(if first { marker_extent / 2.0 } else { 0.0 }),
                    bottom: Some(if last { marker_extent / 2.0 } else { 0.0 }),
                    width: Some(connector_width),
                    child: Some(Container::new(Spacer::default()).bg(connector_color).into()),
                    ..Default::default()
                });
                let marker = entry.marker.unwrap_or_else(|| {
                    Container::new(Spacer::default())
                        .size(marker_extent, marker_extent)
                        .border_radius(marker_extent / 2.0)
                        .bg(theme.dot_color)
                        .into()
                });
                let mut rail_children = Vec::with_capacity(2);
                if let Some(connector) = connector {
                    rail_children.push(connector.into());
                }
                rail_children.push(
                    Positioned {
                        left: Some((marker_width - marker_extent) / 2.0),
                        top: Some(0.0),
                        width: Some(marker_extent),
                        height: Some(marker_extent),
                        child: Some(marker),
                        ..Default::default()
                    }
                    .into(),
                );

                Row {
                    id: Some(entry.id),
                    gap: Some(component.content_gap),
                    align_items: AlignItems::Stretch,
                    children: vec![
                        Container::new(ZStack {
                            children: rail_children,
                            ..Default::default()
                        })
                        .width(marker_width)
                        .min_height(marker_extent)
                        .into(),
                        Container::new(entry.child)
                            .padding([0.0, 0.0, 0.0, if last { 0.0 } else { component.item_gap }])
                            .flex_grow(1.0)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into()
            })
            .collect();

        fission_core::ui::Column {
            id: component.id,
            gap: Some(0.0),
            children,
            ..Default::default()
        }
        .into()
    }
}
