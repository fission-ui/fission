use crate::Icon;
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Row, SemanticsRegion, Text, Widget,
};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use fission_ir::{Role, Semantics};
use serde::{Deserialize, Serialize};

const IMPLICIT_BREADCRUMB_ID_SALT: u32 = 0x4252_4541;
const LEGACY_ITEM_ID_PATH: u32 = 0x4954_454d;
const SEPARATOR_ID_PATH: u32 = 0x5345_5041;

/// One segment of a [`Breadcrumb`] navigation trail.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreadcrumbItem {
    /// Segment label.
    pub label: String,
    /// Optional navigation action; the final segment is rendered as current location.
    pub on_click: Option<ActionEnvelope>,
}

/// Ordered navigation trail from a broad parent to the current location.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Breadcrumb {
    /// Segments in ancestor-to-current order.
    pub items: Vec<BreadcrumbItem>,
}

/// One retained segment in a [`BreadcrumbLayout`].
///
/// The accessible `label` remains authoritative when `child` uses richer
/// presentation such as an icon, badge, or independently styled text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreadcrumbEntry {
    /// Stable identity for this segment's current-location or navigation semantics.
    pub id: WidgetId,
    /// Accessible segment label.
    pub label: String,
    /// Retained visible presentation.
    pub child: Widget,
    /// Optional navigation action. The final entry remains the current location.
    pub on_click: Option<ActionEnvelope>,
}

impl BreadcrumbEntry {
    /// Creates a retained, non-actionable segment.
    pub fn new(id: WidgetId, label: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            id,
            label: label.into(),
            child: child.into(),
            on_click: None,
        }
    }

    /// Makes this non-final segment navigable.
    pub fn on_click(mut self, action: ActionEnvelope) -> Self {
        self.on_click = Some(action);
        self
    }
}

/// Retained breadcrumb anatomy with customizable segment and separator content.
///
/// The last entry always represents the current location, even when it carries
/// an action. Earlier entries dispatch their configured navigation action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreadcrumbLayout {
    /// Optional stable identity for the complete trail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<WidgetId>,
    /// Ordered retained segments.
    pub entries: Vec<BreadcrumbEntry>,
    /// Independently retained separators in boundary order.
    ///
    /// Missing entries use the standard layout direction-aware chevron.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub separators: Vec<Widget>,
    /// Gap between every segment and separator.
    pub gap: f32,
}

impl BreadcrumbLayout {
    /// Creates a retained trail with the standard separator and spacing.
    pub fn new(entries: Vec<BreadcrumbEntry>) -> Self {
        Self {
            id: None,
            entries,
            separators: Vec::new(),
            gap: 8.0,
        }
    }

    /// Assigns a stable identity to the complete trail.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Supplies independently constructed retained separators in boundary order.
    ///
    /// When fewer separators than boundaries are supplied, remaining boundaries
    /// use the standard direction-aware chevron.
    pub fn separators(mut self, separators: Vec<Widget>) -> Self {
        self.separators = separators;
        self
    }

    /// Sets the gap between every segment and separator.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }
}

impl From<Breadcrumb> for Widget {
    fn from(component: Breadcrumb) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let root_id = fission_core::build::next_implicit_widget_id(IMPLICIT_BREADCRUMB_ID_SALT)
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.breadcrumb"));
        let item_count = component.items.len();
        let entries = component
            .items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let label = item.label;
                let is_last = index + 1 == item_count;
                BreadcrumbEntry {
                    id: WidgetId::derived(root_id.as_u128(), &[LEGACY_ITEM_ID_PATH, index as u32]),
                    child: Text::new(label.clone())
                        .color(if is_last {
                            view.env().theme.tokens.colors.text_primary
                        } else {
                            view.env().theme.tokens.colors.text_secondary
                        })
                        .flex_shrink(0.0)
                        .into(),
                    label,
                    on_click: item.on_click,
                }
            })
            .collect();

        BreadcrumbLayout::new(entries).id(root_id).into()
    }
}

impl From<BreadcrumbLayout> for Widget {
    fn from(component: BreadcrumbLayout) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let root_id = component
            .id
            .or_else(|| fission_core::build::next_implicit_widget_id(IMPLICIT_BREADCRUMB_ID_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.breadcrumb.layout"));
        let entry_count = component.entries.len();
        let mut separators = component.separators.into_iter();
        let mut children = Vec::new();

        for (index, entry) in component.entries.into_iter().enumerate() {
            let is_last = index + 1 == entry_count;

            if index > 0 {
                let separator = separators.next().unwrap_or_else(|| {
                    Icon::svg_directional(
                        material::navigation::chevron_right::regular(),
                        material::navigation::chevron_left::regular(),
                    )
                    .size(16.0)
                    .color(tokens.colors.text_secondary)
                    .into()
                });
                let mut separator_wrapper = Container::new(separator);
                separator_wrapper.id = Some(WidgetId::derived(
                    root_id.as_u128(),
                    &[SEPARATOR_ID_PATH, index as u32],
                ));
                separator_wrapper.flex_shrink = 0.0;
                children.push(separator_wrapper.into());
            }

            if !is_last && entry.on_click.is_some() {
                children.push(
                    Container::new(Button {
                        id: Some(entry.id),
                        variant: ButtonVariant::Ghost,
                        content_align: ButtonContentAlign::Start,
                        child: Some(entry.child),
                        on_press: entry.on_click,
                        semantics: Some(Semantics {
                            role: Role::Button,
                            label: Some(entry.label),
                            focusable: true,
                            ..Default::default()
                        }),
                        ..Default::default()
                    })
                    .flex_shrink(0.0)
                    .into(),
                );
            } else {
                let semantic_entry: Widget = SemanticsRegion {
                    id: Some(entry.id),
                    label: Some(entry.label),
                    role: Role::Text,
                    focusable: Some(false),
                    child: Some(entry.child),
                    ..Default::default()
                }
                .into();
                let mut wrapper = Container::new(semantic_entry).flex_shrink(0.0);
                if is_last {
                    wrapper = wrapper.flex_grow(1.0);
                }
                children.push(wrapper.into());
            }
        }

        Row {
            id: Some(root_id),
            gap: Some(component.gap),
            align_items: fission_ir::op::AlignItems::Center,
            children,
            ..Default::default()
        }
        .into()
    }
}
