use crate::popover::{popover_with_options, Popover};
use crate::stack::{HStack, VStack};
use crate::{FlyoutAlignment, FlyoutOptions, Icon};
use fission_core::internal::{InternalIrBuilder, InternalLowerer, InternalLoweringCx};
use fission_core::op::{
    AlignItems, BoxAlignment, BoxStyle, Fill, FlexDirection, FlexWrap, JustifyContent, LayoutOp,
    Length, Op, PaintOp, Stroke,
};
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonStyleOverride, ButtonVariant, ComponentSize, Container,
    Scroll, SemanticsRegion, Spacer, Text, TextContent, Widget,
};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use fission_ir::semantics::ActionTrigger;
use fission_ir::{
    ActionEntry, ActionSet, CompositeScalar, CompositeStyle, LayoutDirection, PopupKind, Role,
    SemanticOrientation, Semantics,
};
use fission_theme::{ComponentState, MenuTheme, ResolvedComponentStyle};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DEFAULT_MENU_MAX_HEIGHT: f32 = 300.0;
const POPUP_ID_PATH: &[u32] = &[1];
// Keep explicit anatomy identities outside the popup's ordinary structural paths.
const ITEM_ID_PATH: u32 = 0x4D45_4E55;
const GROUP_ID_PATH: u32 = 0x4752_4f55;
const LABEL_ID_PATH: u32 = 0x4c41_424c;
const SEPARATOR_ID_PATH: u32 = 0x5345_5052;
const DESCRIPTION_ID_PATH: u32 = 0x4445_5343;
const CUSTOM_WRAPPER_ID_PATH: u32 = 0x5752_4150;
const IMPLICIT_MENU_CONTENT_ID_SALT: u32 = 0x4d43_4e54;
const IMPLICIT_MENU_GROUP_ID_SALT: u32 = 0x4d47_5250;
const IMPLICIT_MENU_SEPARATOR_ID_SALT: u32 = 0x4d53_4550;
const IMPLICIT_MENU_ITEM_ID_SALT: u32 = 0x4d49_544d;

const fn enabled_for_sequential_focus() -> bool {
    true
}

pub(crate) fn menu_surface_item_id(
    popup_id: WidgetId,
    index: usize,
    semantics_identifier: Option<&str>,
) -> WidgetId {
    semantics_identifier.map_or_else(
        || WidgetId::derived(popup_id.as_u128(), &[ITEM_ID_PATH, index as u32]),
        |identifier| WidgetId::scoped(popup_id.as_u128(), identifier),
    )
}

/// A single entry in a [`Menu`]: label text, optional SVG icon, and selection action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuItem {
    /// Text displayed for the menu entry.
    pub label: String,
    /// Optional SVG markup displayed before the label.
    pub icon: Option<String>,
    /// Action dispatched when the entry is selected.
    pub on_select: Option<ActionEnvelope>,
    /// Stable identifier exposed on this entry's actionable semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

/// Visual and semantic tone of a composed menu action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuItemTone {
    /// Standard command or option treatment.
    #[default]
    Normal,
    /// Irreversible or data-removing command treatment.
    Destructive,
}

/// Content displayed at the trailing edge of a composed menu action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuItemTrailing {
    /// Keyboard shortcut hint, such as `Ctrl+K`.
    Shortcut(String),
    /// Supporting value or other non-shortcut metadata.
    Metadata(String),
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
enum MenuActionRole {
    #[default]
    MenuItem,
    Option,
}

impl MenuActionRole {
    fn semantics_role(self) -> Role {
        match self {
            Self::MenuItem => Role::MenuItem,
            Self::Option => Role::Option,
        }
    }
}

/// Retained actionable-row anatomy shared by menus and list-box popups.
///
/// Use the builder methods to add optional anatomy without making mutually
/// exclusive trailing content ambiguous. [`Menu`], [`MenuButton`],
/// [`crate::Select`], and [`crate::Combobox`] all adapt their legacy item models
/// into this same retained implementation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuActionItem {
    /// Explicit stable identity. A containing [`MenuContent`] derives one when omitted.
    pub id: Option<WidgetId>,
    /// Primary action label.
    pub label: String,
    /// Optional supporting line beneath the label.
    pub description: Option<String>,
    /// Optional SVG markup displayed at the leading edge.
    pub leading_icon: Option<String>,
    /// Optional shortcut or metadata displayed at the trailing edge.
    pub trailing: Option<MenuItemTrailing>,
    /// Action dispatched when the enabled item is selected.
    pub on_select: Option<ActionEnvelope>,
    /// Stable identifier exposed by the actionable semantics node.
    pub semantics_identifier: Option<String>,
    /// Selection state. `None` denotes a command rather than a selectable option.
    pub selected: Option<bool>,
    /// Disabled items remain visible but cannot receive focus or dispatch actions.
    pub disabled: bool,
    /// Normal or destructive presentation.
    pub tone: MenuItemTone,
    #[serde(default)]
    role: MenuActionRole,
    #[serde(default = "enabled_for_sequential_focus")]
    sequential_focusable: bool,
    #[serde(skip)]
    reserve_leading_space: bool,
    #[serde(skip)]
    reserve_indicator_space: bool,
    #[serde(skip)]
    width: Option<f32>,
}

impl MenuActionItem {
    /// Creates a normal enabled command with no optional anatomy.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: None,
            label: label.into(),
            description: None,
            leading_icon: None,
            trailing: None,
            on_select: None,
            semantics_identifier: None,
            selected: None,
            disabled: false,
            tone: MenuItemTone::Normal,
            role: MenuActionRole::MenuItem,
            sequential_focusable: true,
            reserve_leading_space: false,
            reserve_indicator_space: false,
            width: None,
        }
    }

    /// Creates a selectable list-box option with an explicit selected state.
    pub fn option(label: impl Into<String>, selected: bool) -> Self {
        let mut item = Self::new(label);
        item.role = MenuActionRole::Option;
        item.selected = Some(selected);
        item
    }

    /// Uses an explicit stable identity for interaction, focus, and selection.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Adds supporting text beneath the primary label.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds an SVG icon at the logical leading edge.
    pub fn leading_icon(mut self, svg: impl Into<String>) -> Self {
        self.leading_icon = Some(svg.into());
        self
    }

    /// Shows a keyboard shortcut hint at the logical trailing edge.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.trailing = Some(MenuItemTrailing::Shortcut(shortcut.into()));
        self
    }

    /// Shows non-shortcut metadata at the logical trailing edge.
    pub fn metadata(mut self, metadata: impl Into<String>) -> Self {
        self.trailing = Some(MenuItemTrailing::Metadata(metadata.into()));
        self
    }

    /// Dispatches `action` when the enabled item is activated.
    pub fn on_select(mut self, action: ActionEnvelope) -> Self {
        self.on_select = Some(action);
        self
    }

    /// Exposes a stable identifier to accessibility and semantic tests.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    /// Marks the item as selected and reserves the shared indicator column.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Keeps the item visible while removing activation and focus behavior.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Selects the item's semantic visual tone.
    pub fn tone(mut self, tone: MenuItemTone) -> Self {
        self.tone = tone;
        self
    }

    pub(crate) fn sequential_focusable(mut self, sequential_focusable: bool) -> Self {
        self.sequential_focusable = sequential_focusable;
        self
    }
}

impl From<MenuItem> for MenuActionItem {
    fn from(item: MenuItem) -> Self {
        Self {
            label: item.label,
            leading_icon: item.icon,
            on_select: item.on_select,
            semantics_identifier: item.semantics_identifier,
            ..Self::new(String::new())
        }
    }
}

/// Retained text heading for a group of related menu entries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuLabel {
    /// Explicit stable identity. A containing surface or group derives one when omitted.
    pub id: Option<WidgetId>,
    /// Visible heading text.
    pub text: String,
    /// Stable identifier exposed by the label semantics node.
    pub semantics_identifier: Option<String>,
}

impl MenuLabel {
    /// Creates a visible, non-focusable menu heading.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: None,
            text: text.into(),
            semantics_identifier: None,
        }
    }

    /// Uses an explicit stable identity for semantic relationships.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Exposes a stable identifier to accessibility and semantic tests.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

/// Retained visual boundary between menu regions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MenuSeparator {
    /// Explicit stable identity. A containing [`MenuContent`] derives one when omitted.
    pub id: Option<WidgetId>,
}

impl MenuSeparator {
    /// Creates a non-focusable visual separator.
    pub const fn new() -> Self {
        Self { id: None }
    }

    /// Uses an explicit stable identity for the retained separator.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Retained group of related menu entries with an optional visible heading.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuGroup {
    /// Explicit stable identity. A containing [`MenuContent`] derives one when omitted.
    pub id: Option<WidgetId>,
    /// Optional visible heading that labels the group.
    pub label: Option<MenuLabel>,
    /// Entries contained by this group in display order.
    pub entries: Vec<MenuEntry>,
}

impl MenuGroup {
    /// Creates an unlabeled group of related entries.
    pub fn new(entries: Vec<MenuEntry>) -> Self {
        Self {
            id: None,
            label: None,
            entries,
        }
    }

    /// Uses an explicit stable identity for semantic relationships.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Adds a visible heading that labels this group.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(MenuLabel::new(label));
        self
    }
}

/// One retained entry in a composed [`MenuContent`] surface.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MenuEntry {
    /// An actionable command or selectable option.
    Item(MenuActionItem),
    /// A related group with an optional visible heading.
    Group(MenuGroup),
    /// A standalone non-focusable heading.
    Label(MenuLabel),
    /// An explicit visual boundary between regions.
    Separator(MenuSeparator),
}

impl From<MenuActionItem> for MenuEntry {
    fn from(value: MenuActionItem) -> Self {
        Self::Item(value)
    }
}

impl From<MenuItem> for MenuEntry {
    fn from(value: MenuItem) -> Self {
        Self::Item(value.into())
    }
}

impl From<MenuGroup> for MenuEntry {
    fn from(value: MenuGroup) -> Self {
        Self::Group(value)
    }
}

impl From<MenuLabel> for MenuEntry {
    fn from(value: MenuLabel) -> Self {
        Self::Label(value)
    }
}

impl From<MenuSeparator> for MenuEntry {
    fn from(value: MenuSeparator) -> Self {
        Self::Separator(value)
    }
}

impl From<MenuEntry> for Widget {
    fn from(entry: MenuEntry) -> Self {
        match entry {
            MenuEntry::Item(item) => item.into(),
            MenuEntry::Group(group) => group.into(),
            MenuEntry::Label(label) => label.into(),
            MenuEntry::Separator(separator) => separator.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum MenuContentKind {
    #[default]
    Menu,
    ListBox,
}

impl MenuContentKind {
    fn role(self) -> Role {
        match self {
            Self::Menu => Role::Menu,
            Self::ListBox => Role::ListBox,
        }
    }
}

/// Retained, themed popup surface for composed menu anatomy.
///
/// This type is useful when a command surface needs groups, labels,
/// separators, richer item text, trailing metadata, or destructive actions.
/// The simpler [`Menu`] remains source-compatible and lowers into this type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuContent {
    /// Explicit stable identity for the composite surface.
    pub id: Option<WidgetId>,
    /// Retained anatomy entries in display order.
    pub entries: Vec<MenuEntry>,
    /// Optional logical surface width.
    pub width: Option<f32>,
    /// Optional maximum logical height before content scrolls.
    pub max_height: Option<f32>,
    #[serde(default)]
    fluid_width: bool,
    #[serde(default)]
    kind: MenuContentKind,
    #[serde(default)]
    on_dismiss: Option<ActionEnvelope>,
    #[serde(default)]
    focus_barrier: bool,
}

impl MenuContent {
    /// Creates a vertical command-menu surface from retained anatomy entries.
    pub fn new(entries: Vec<MenuEntry>) -> Self {
        Self {
            id: None,
            entries,
            width: None,
            max_height: None,
            fluid_width: false,
            kind: MenuContentKind::Menu,
            on_dismiss: None,
            focus_barrier: false,
        }
    }

    /// Creates a list-box surface whose actionable entries use option semantics.
    ///
    /// Action items are normalized to option semantics and an explicit
    /// unselected state when needed. The surface leaves width resolution to its
    /// retained flyout so it can match an anchor.
    pub fn list_box(mut entries: Vec<MenuEntry>) -> Self {
        mark_entries_as_options(&mut entries);
        let mut content = Self::new(entries);
        content.kind = MenuContentKind::ListBox;
        content.fluid_width = true;
        content
    }

    /// Uses an explicit stable identity for the composite surface.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Uses a fixed logical surface width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Bounds the surface and enables vertical scrolling when needed.
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = Some(max_height);
        self
    }

    /// Lets a containing flyout own width resolution when `true`.
    pub fn fluid_width(mut self, fluid_width: bool) -> Self {
        self.fluid_width = fluid_width;
        self
    }

    /// Adds the composite dismissal action used by Escape and equivalent input.
    pub fn dismiss_action(mut self, action: Option<ActionEnvelope>) -> Self {
        self.on_dismiss = action;
        self
    }

    /// Traps focus inside the surface while it is mounted when `true`.
    pub fn focus_barrier(mut self, focus_barrier: bool) -> Self {
        self.focus_barrier = focus_barrier;
        self
    }
}

/// A vertical dropdown menu rendered as a scrollable list of [`MenuItem`] entries.
///
/// The menu is displayed inside a compact bordered surface. When the total item
/// height exceeds `max_height` (default 300px), the rows scroll within that
/// bounded surface.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Menu {
    /// Menu entries in display order.
    pub items: Vec<MenuItem>,
    /// Optional logical width; defaults to the design-system menu width.
    pub width: Option<f32>,
    /// Optional maximum logical height before vertical scrolling is enabled.
    pub max_height: Option<f32>,
}

impl From<Menu> for Widget {
    fn from(component: Menu) -> Self {
        let id = fission_core::build::current_widget_id();
        let mut content = MenuContent::new(
            component
                .items
                .into_iter()
                .map(MenuActionItem::from)
                .map(MenuEntry::Item)
                .collect(),
        );
        content.id = id;
        content.width = component.width;
        content.max_height = component.max_height;
        content.into()
    }
}

impl From<MenuContent> for Widget {
    fn from(mut component: MenuContent) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.menu;
        let surface_style = &theme.surface_style;
        let popup_id = component
            .id
            .or_else(fission_core::build::current_widget_id)
            .or_else(|| fission_core::build::next_implicit_widget_id(IMPLICIT_MENU_CONTENT_ID_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.menu.content"));

        let reserve_leading = entries_have_leading_icon(&component.entries);
        let reserve_indicator = entries_have_selection(&component.entries);
        normalize_sequential_focus(
            &mut component.entries,
            component.kind == MenuContentKind::ListBox,
        );
        let mut item_index = 0;
        let mut structural_index = 0;
        prepare_entries(
            &mut component.entries,
            popup_id,
            &mut item_index,
            &mut structural_index,
            reserve_leading,
            reserve_indicator,
            None,
        );

        let surface_gap = surface_style.gap.unwrap_or(0.0);
        let content_height = entries_height(&component.entries, theme, surface_gap);
        let padding = surface_style.padding_box(4.0, 4.0);
        let maximum_height = component
            .max_height
            .unwrap_or(DEFAULT_MENU_MAX_HEIGHT)
            .max(padding[2] + padding[3]);
        let viewport_height = content_height.min(maximum_height - padding[2] - padding[3]);
        let popup_height = viewport_height + padding[2] + padding[3];
        let popup_width = component.width.or_else(|| {
            (!component.fluid_width)
                .then(|| surface_style.width)
                .flatten()
        });
        let content_width = popup_width.map(|width| (width - padding[0] - padding[1]).max(0.0));

        for entry in &mut component.entries {
            entry.set_width(content_width);
        }
        let entry_widgets = component.entries.into_iter().map(Widget::from).collect();
        let scrollable_content: Widget = Scroll {
            child: Some(
                VStack {
                    spacing: Some(surface_gap),
                    children: entry_widgets,
                }
                .into(),
            ),
            height: Some(viewport_height),
            width: content_width,
            show_scrollbar: content_height > viewport_height + 0.5,
            ..Default::default()
        }
        .into();

        let mut surface = Container::new(scrollable_content)
            .height(popup_height)
            .padding(padding)
            .border_radius(surface_style.radius.unwrap_or(0.0))
            .shadows(
                surface_style
                    .shadows
                    .iter()
                    .map(fission_theme::ShadowLayer::to_box_shadow)
                    .collect(),
            );
        if let Some(width) = popup_width {
            surface = surface.width(width.max(padding[0] + padding[1]));
        }
        if let Some(fill) = &surface_style.background {
            surface = surface.bg_fill(fill.clone());
        }
        if let Some(border) = &surface_style.border {
            if let Fill::Solid(color) = &border.fill {
                surface = surface.border(*color, border.width);
            }
        }
        if let Some(dash) = &surface_style.border_dash {
            surface = surface.border_dash(dash.clone());
        }

        let mut popup = SemanticsRegion::new(surface)
            .role(component.kind.role())
            .orientation(SemanticOrientation::Vertical)
            .sequential_focusable(false);
        popup.id = Some(popup_id);
        if let Some(action) = component.on_dismiss {
            popup = popup.dismiss_action(action);
        }
        if component.focus_barrier {
            popup = popup.focus_scope(true);
        }
        popup.into()
    }
}

impl MenuEntry {
    fn set_width(&mut self, width: Option<f32>) {
        match self {
            MenuEntry::Item(item) => item.width = width,
            MenuEntry::Group(group) => {
                for entry in &mut group.entries {
                    entry.set_width(width);
                }
            }
            MenuEntry::Label(_) | MenuEntry::Separator(_) => {}
        }
    }

    fn estimated_height(&self, theme: &MenuTheme, gap: f32) -> f32 {
        match self {
            MenuEntry::Item(item) => item.estimated_height(theme),
            MenuEntry::Group(group) => group.estimated_height(theme, gap),
            MenuEntry::Label(_) => text_style_height(&theme.group_label_style),
            MenuEntry::Separator(_) => theme.separator_style.height.unwrap_or(1.0),
        }
    }
}

impl MenuActionItem {
    fn estimated_height(&self, theme: &MenuTheme) -> f32 {
        let base = &theme.item_states.default;
        let padding = base.padding_box(8.0, 6.0);
        let label_height = base.line_height.or(base.font_size).unwrap_or(20.0);
        let text_height = self.description.as_ref().map_or(label_height, |_| {
            label_height
                + theme.description_style.gap.unwrap_or(2.0)
                + theme
                    .description_style
                    .line_height
                    .or(theme.description_style.font_size)
                    .unwrap_or(18.0)
        });
        base.height
            .unwrap_or(0.0)
            .max(text_height + padding[2] + padding[3])
    }
}

impl MenuGroup {
    fn estimated_height(&self, theme: &MenuTheme, gap: f32) -> f32 {
        let label_height = self
            .label
            .as_ref()
            .map(|_| text_style_height(&theme.group_label_style));
        let entry_height = entries_height(&self.entries, theme, gap);
        label_height.unwrap_or(0.0)
            + entry_height
            + if label_height.is_some() && !self.entries.is_empty() {
                gap
            } else {
                0.0
            }
    }
}

fn text_style_height(style: &ResolvedComponentStyle) -> f32 {
    let padding = style.padding_box(0.0, 0.0);
    style.height.unwrap_or_else(|| {
        style.line_height.or(style.font_size).unwrap_or(0.0) + padding[2] + padding[3]
    })
}

fn entries_height(entries: &[MenuEntry], theme: &MenuTheme, gap: f32) -> f32 {
    let children = entries
        .iter()
        .map(|entry| entry.estimated_height(theme, gap))
        .sum::<f32>();
    children + entries.len().saturating_sub(1) as f32 * gap
}

fn entries_have_leading_icon(entries: &[MenuEntry]) -> bool {
    entries.iter().any(|entry| match entry {
        MenuEntry::Item(item) => item.leading_icon.is_some(),
        MenuEntry::Group(group) => entries_have_leading_icon(&group.entries),
        MenuEntry::Label(_) | MenuEntry::Separator(_) => false,
    })
}

fn entries_have_selection(entries: &[MenuEntry]) -> bool {
    entries.iter().any(|entry| match entry {
        MenuEntry::Item(item) => item.selected.is_some(),
        MenuEntry::Group(group) => entries_have_selection(&group.entries),
        MenuEntry::Label(_) | MenuEntry::Separator(_) => false,
    })
}

fn mark_entries_as_options(entries: &mut [MenuEntry]) {
    for entry in entries {
        match entry {
            MenuEntry::Item(item) => {
                item.role = MenuActionRole::Option;
                item.selected.get_or_insert(false);
            }
            MenuEntry::Group(group) => mark_entries_as_options(&mut group.entries),
            MenuEntry::Label(_) | MenuEntry::Separator(_) => {}
        }
    }
}

fn normalize_sequential_focus(entries: &mut [MenuEntry], prefer_selected: bool) {
    let mut index = 0;
    let mut first_enabled = None;
    let mut first_selected = None;
    find_sequential_focus_candidates(entries, &mut index, &mut first_enabled, &mut first_selected);
    let target = if prefer_selected {
        first_selected.or(first_enabled)
    } else {
        first_enabled
    };
    let mut index = 0;
    apply_sequential_focus_target(entries, target, &mut index);
}

fn find_sequential_focus_candidates(
    entries: &[MenuEntry],
    index: &mut usize,
    first_enabled: &mut Option<usize>,
    first_selected: &mut Option<usize>,
) {
    for entry in entries {
        match entry {
            MenuEntry::Item(item) => {
                if item.sequential_focusable && !item.disabled {
                    first_enabled.get_or_insert(*index);
                    if item.selected == Some(true) {
                        first_selected.get_or_insert(*index);
                    }
                }
                *index += 1;
            }
            MenuEntry::Group(group) => find_sequential_focus_candidates(
                &group.entries,
                index,
                first_enabled,
                first_selected,
            ),
            MenuEntry::Label(_) | MenuEntry::Separator(_) => {}
        }
    }
}

fn apply_sequential_focus_target(
    entries: &mut [MenuEntry],
    target: Option<usize>,
    index: &mut usize,
) {
    for entry in entries {
        match entry {
            MenuEntry::Item(item) => {
                item.sequential_focusable =
                    item.sequential_focusable && !item.disabled && target == Some(*index);
                *index += 1;
            }
            MenuEntry::Group(group) => {
                apply_sequential_focus_target(&mut group.entries, target, index)
            }
            MenuEntry::Label(_) | MenuEntry::Separator(_) => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_entries(
    entries: &mut [MenuEntry],
    popup_id: WidgetId,
    item_index: &mut usize,
    structural_index: &mut usize,
    reserve_leading: bool,
    reserve_indicator: bool,
    width: Option<f32>,
) {
    for entry in entries {
        match entry {
            MenuEntry::Item(item) => {
                item.id = Some(item.id.unwrap_or_else(|| {
                    menu_surface_item_id(
                        popup_id,
                        *item_index,
                        item.semantics_identifier.as_deref(),
                    )
                }));
                item.reserve_leading_space |= reserve_leading;
                item.reserve_indicator_space |= reserve_indicator;
                if width.is_some() {
                    item.width = width;
                }
                *item_index += 1;
            }
            MenuEntry::Group(group) => {
                let group_id = group.id.unwrap_or_else(|| {
                    WidgetId::derived(
                        popup_id.as_u128(),
                        &[GROUP_ID_PATH, *structural_index as u32],
                    )
                });
                group.id = Some(group_id);
                *structural_index += 1;
                if let Some(label) = &mut group.label {
                    label.id = Some(label.id.unwrap_or_else(|| {
                        WidgetId::derived(group_id.as_u128(), &[LABEL_ID_PATH])
                    }));
                }
                prepare_entries(
                    &mut group.entries,
                    popup_id,
                    item_index,
                    structural_index,
                    reserve_leading,
                    reserve_indicator,
                    width,
                );
            }
            MenuEntry::Label(label) => {
                label.id = Some(label.id.unwrap_or_else(|| {
                    WidgetId::derived(
                        popup_id.as_u128(),
                        &[LABEL_ID_PATH, *structural_index as u32],
                    )
                }));
                *structural_index += 1;
            }
            MenuEntry::Separator(separator) => {
                separator.id = Some(separator.id.unwrap_or_else(|| {
                    WidgetId::derived(
                        popup_id.as_u128(),
                        &[SEPARATOR_ID_PATH, *structural_index as u32],
                    )
                }));
                *structural_index += 1;
            }
        }
    }
}

impl From<MenuGroup> for Widget {
    fn from(mut group: MenuGroup) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let gap = view
            .env()
            .theme
            .components
            .menu
            .surface_style
            .gap
            .unwrap_or(0.0);
        let group_id = group
            .id
            .or_else(fission_core::build::current_widget_id)
            .or_else(|| fission_core::build::next_implicit_widget_id(IMPLICIT_MENU_GROUP_ID_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.menu.group"));
        group.id = Some(group_id);
        let reserve_leading = entries_have_leading_icon(&group.entries);
        let reserve_indicator = entries_have_selection(&group.entries);
        let mut item_index = 0;
        let mut structural_index = 0;
        prepare_entries(
            &mut group.entries,
            group_id,
            &mut item_index,
            &mut structural_index,
            reserve_leading,
            reserve_indicator,
            None,
        );

        let mut labelled_by = Vec::new();
        let mut children =
            Vec::with_capacity(group.entries.len() + usize::from(group.label.is_some()));
        if let Some(mut label) = group.label {
            let label_id = label
                .id
                .unwrap_or_else(|| WidgetId::derived(group_id.as_u128(), &[LABEL_ID_PATH]));
            label.id = Some(label_id);
            labelled_by.push(label_id);
            children.push(label.into());
        }
        children.extend(group.entries.into_iter().map(Widget::from));

        SemanticsRegion {
            id: Some(group_id),
            role: Role::Group,
            labelled_by,
            focusable: Some(false),
            child: Some(
                VStack {
                    spacing: Some(gap),
                    children,
                }
                .into(),
            ),
            ..Default::default()
        }
        .into()
    }
}

impl From<MenuLabel> for Widget {
    fn from(label: MenuLabel) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let style = &view.env().theme.components.menu.group_label_style;
        let color = style
            .text_color
            .unwrap_or(view.env().theme.tokens.colors.text_muted);
        let mut text = Text::new(label.text.clone())
            .size(style.font_size.unwrap_or(12.0))
            .weight(style.font_weight.unwrap_or(500))
            .color(color);
        if let Some(line_height) = style.line_height {
            text = text.line_height(line_height);
        }
        if let Some(letter_spacing) = style.letter_spacing {
            text = text.letter_spacing(letter_spacing);
        }
        let mut container = Container::new(text)
            .padding(style.padding_box(0.0, 0.0))
            .border_radius(style.radius.unwrap_or(0.0));
        if let Some(height) = style.height {
            container = container.height(height);
        }
        if let Some(fill) = &style.background {
            container = container.bg_fill(fill.clone());
        }

        SemanticsRegion {
            id: label.id,
            identifier: label.semantics_identifier,
            label: Some(label.text),
            role: Role::Text,
            focusable: Some(false),
            child: Some(container.into()),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct MenuSeparatorLowerer {
    id: WidgetId,
    style: ResolvedComponentStyle,
}

impl From<MenuSeparator> for Widget {
    fn from(separator: MenuSeparator) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let id = separator
            .id
            .or_else(fission_core::build::current_widget_id)
            .or_else(|| {
                fission_core::build::next_implicit_widget_id(IMPLICIT_MENU_SEPARATOR_ID_SALT)
            })
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.menu.separator"));
        fission_core::internal::custom_render_widget(fission_core::internal::InternalRenderNode {
            debug_tag: "Menu.Separator".into(),
            lowerer: Some(Arc::new(MenuSeparatorLowerer {
                id,
                style: view.env().theme.components.menu.separator_style.clone(),
            })),
            render_object: None,
        })
    }
}

impl InternalLowerer for MenuSeparatorLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let border = self.style.border.clone();
        let line_height = border.as_ref().map(|border| border.width).unwrap_or(1.0);
        let line_fill = border.map(|border| border.fill);
        let padding = self.style.padding_box(0.0, 4.0);
        let margin = self.style.margin.unwrap_or([0.0; 4]);

        cx.push_scope(self.id);
        let mut line = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(Length::percent(100.0)),
                    height: Some(Length::Points(line_height)),
                    alignment: BoxAlignment::Stretch,
                    ..Default::default()
                },
                flex_grow: 1.0,
                flex_shrink: 0.0,
            }),
        );
        if let Some(fill) = line_fill {
            line.add_child(
                InternalIrBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: Some(fill),
                        stroke: None,
                        corner_radius: 0.0,
                        shadow: None,
                    }),
                )
                .build(cx),
            );
        }
        let line_id = line.build(cx);
        let mut positioned_line = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::PositionedLengths {
                left: Some(Length::Points(margin[0])),
                top: Some(Length::Points(padding[2] + margin[2])),
                right: Some(Length::Points(margin[1])),
                bottom: Some(Length::Points(padding[3] + margin[3])),
                width: None,
                height: None,
            }),
        );
        positioned_line.add_child(line_id);
        let positioned_line_id = positioned_line.build(cx);

        let mut outer = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::StyledBox {
                style: BoxStyle {
                    height: self.style.height.map(Length::Points),
                    alignment: BoxAlignment::Center,
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
        );
        outer.add_child(positioned_line_id);
        let outer_id = outer.build(cx);
        let mut semantics = InternalIrBuilder::new(
            self.id,
            Op::Semantics(Semantics {
                role: Role::Separator,
                focusable: false,
                sequential_focusable: false,
                ..Default::default()
            }),
        );
        semantics.add_child(outer_id);
        let id = semantics.build(cx);
        cx.pop_scope();
        id
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(WidgetId::derived(
            self.id.as_u128(),
            &[CUSTOM_WRAPPER_ID_PATH],
        ))
    }

    fn stable_key(&self) -> u64 {
        stable_id_key(self.id)
    }
}

#[derive(Clone, Debug)]
struct MenuActionItemLowerer {
    id: WidgetId,
    label: String,
    description: Option<String>,
    leading_icon: Option<String>,
    trailing: Option<MenuItemTrailing>,
    on_select: Option<ActionEnvelope>,
    semantics_identifier: Option<String>,
    selected: Option<bool>,
    disabled: bool,
    role: Role,
    sequential_focusable: bool,
    reserve_leading_space: bool,
    reserve_indicator_space: bool,
    width: Option<f32>,
    style: ResolvedComponentStyle,
    description_style: ResolvedComponentStyle,
    shortcut_style: ResolvedComponentStyle,
    metadata_style: ResolvedComponentStyle,
    indicator_style: ResolvedComponentStyle,
}

impl From<MenuActionItem> for Widget {
    fn from(item: MenuActionItem) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let id = item
            .id
            .or_else(fission_core::build::current_widget_id)
            .or_else(|| fission_core::build::next_implicit_widget_id(IMPLICIT_MENU_ITEM_ID_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.menu.item"));
        let theme = &view.env().theme.components.menu;
        let destructive = item.tone == MenuItemTone::Destructive;
        let interaction = &view.runtime().interaction;
        let mut style = theme.resolve_item(
            destructive,
            if item.selected == Some(true) {
                ComponentState::Selected
            } else {
                ComponentState::Default
            },
        );
        if item.disabled {
            style = theme.resolve_item(destructive, ComponentState::Disabled);
        } else {
            if interaction.is_hovered(id) {
                style = style.merge(&theme.resolve_item(destructive, ComponentState::Hover));
            }
            if interaction.is_pressed(id) {
                style = style.merge(&theme.resolve_item(destructive, ComponentState::Active));
            }
            if interaction.is_focused(id) {
                style = style.merge_composing_shadows(
                    &theme.resolve_item(destructive, ComponentState::Focus),
                );
            }
        }
        let mut shortcut_style = theme.shortcut_style.clone();
        let mut metadata_style = theme.metadata_style.clone();
        if interaction.is_focused(id) && !item.disabled {
            shortcut_style.text_color = style.text_color;
            metadata_style.text_color = style.text_color;
        }

        fission_core::internal::custom_render_widget(fission_core::internal::InternalRenderNode {
            debug_tag: "Menu.ActionItem".into(),
            lowerer: Some(Arc::new(MenuActionItemLowerer {
                id,
                label: item.label,
                description: item.description,
                leading_icon: item.leading_icon,
                trailing: item.trailing,
                on_select: item.on_select,
                semantics_identifier: item.semantics_identifier,
                selected: item.selected,
                disabled: item.disabled,
                role: item.role.semantics_role(),
                sequential_focusable: item.sequential_focusable,
                reserve_leading_space: item.reserve_leading_space,
                reserve_indicator_space: item.reserve_indicator_space,
                width: item.width,
                style,
                description_style: theme.description_style.clone(),
                shortcut_style,
                metadata_style,
                indicator_style: theme.indicator_style.clone(),
            })),
            render_object: None,
        })
    }
}

impl InternalLowerer for MenuActionItemLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        cx.push_scope(self.id);
        let layout_id = cx.next_node_id();
        cx.push_scope(layout_id);

        let text_color = self
            .style
            .text_color
            .unwrap_or(cx.env.theme.tokens.colors.text_primary);
        let icon_size = self.style.icon_size.unwrap_or(16.0);
        let mut row = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
                wrap: FlexWrap::NoWrap,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: Some(self.style.gap.unwrap_or(8.0)),
                line_gap: None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
            }),
        );

        if self.reserve_leading_space || self.leading_icon.is_some() {
            let leading: Widget = self.leading_icon.as_ref().map_or_else(
                || {
                    Spacer {
                        width: Some(icon_size),
                        height: Some(icon_size),
                        ..Default::default()
                    }
                    .into()
                },
                |icon| {
                    Icon::svg(icon.clone())
                        .size(icon_size)
                        .color(text_color)
                        .into()
                },
            );
            row.add_child(fission_core::internal::lower_widget(&leading, cx));
        }

        let mut label = Text::new(self.label.clone())
            .size(self.style.font_size.unwrap_or(14.0))
            .weight(self.style.font_weight.unwrap_or(400))
            .color(text_color);
        if let Some(line_height) = self.style.line_height {
            label = label.line_height(line_height);
        }
        if let Some(letter_spacing) = self.style.letter_spacing {
            label = label.letter_spacing(letter_spacing);
        }
        let label: Widget = label.into();

        let mut text_stack = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                wrap: FlexWrap::NoWrap,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: self
                    .description
                    .as_ref()
                    .map(|_| self.description_style.gap.unwrap_or(2.0)),
                line_gap: None,
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Center,
            }),
        );
        text_stack.add_child(fission_core::internal::lower_widget(&label, cx));

        let description_id = self
            .description
            .as_ref()
            .map(|_| WidgetId::derived(self.id.as_u128(), &[DESCRIPTION_ID_PATH]));
        if let (Some(description), Some(description_id)) = (&self.description, description_id) {
            let description_color = if self.disabled {
                text_color
            } else {
                self.description_style
                    .text_color
                    .unwrap_or(cx.env.theme.tokens.colors.text_muted)
            };
            let mut description_text = Text::new(description.clone())
                .size(self.description_style.font_size.unwrap_or(13.0))
                .weight(self.description_style.font_weight.unwrap_or(400))
                .color(description_color);
            if let Some(line_height) = self.description_style.line_height {
                description_text = description_text.line_height(line_height);
            }
            let description_widget: Widget = SemanticsRegion {
                id: Some(description_id),
                label: Some(description.clone()),
                role: Role::Text,
                focusable: Some(false),
                child: Some(description_text.into()),
                ..Default::default()
            }
            .into();
            text_stack.add_child(fission_core::internal::lower_widget(
                &description_widget,
                cx,
            ));
        }
        row.add_child(text_stack.build(cx));

        if let Some(trailing) = &self.trailing {
            let (value, style) = match trailing {
                MenuItemTrailing::Shortcut(value) => (value, &self.shortcut_style),
                MenuItemTrailing::Metadata(value) => (value, &self.metadata_style),
            };
            let trailing_color = if self.disabled {
                text_color
            } else {
                style
                    .text_color
                    .unwrap_or(cx.env.theme.tokens.colors.text_muted)
            };
            let mut trailing_text = Text::new(value.clone())
                .size(style.font_size.unwrap_or(12.0))
                .weight(style.font_weight.unwrap_or(400))
                .color(trailing_color);
            if let Some(line_height) = style.line_height {
                trailing_text = trailing_text.line_height(line_height);
            }
            if let Some(letter_spacing) = style.letter_spacing {
                trailing_text = trailing_text.letter_spacing(letter_spacing);
            }
            let trailing_widget: Widget = trailing_text.into();
            row.add_child(fission_core::internal::lower_widget(&trailing_widget, cx));
        }

        if self.reserve_indicator_space || self.selected == Some(true) {
            let indicator_size = self.indicator_style.icon_size.unwrap_or(16.0);
            let indicator: Widget = if self.selected == Some(true) {
                Icon::svg(material::navigation::check::regular())
                    .size(indicator_size)
                    .color(self.indicator_style.text_color.unwrap_or(text_color))
                    .into()
            } else {
                Spacer {
                    width: Some(indicator_size),
                    height: Some(indicator_size),
                    ..Default::default()
                }
                .into()
            };
            row.add_child(fission_core::internal::lower_widget(&indicator, cx));
        }

        let row_id = row.build(cx);
        let mut padding = self.style.padding_box(8.0, 6.0);
        if self.reserve_indicator_space || self.selected == Some(true) {
            let inset = self.indicator_style.inset_end.unwrap_or(padding[1]);
            match cx.env.layout_direction {
                LayoutDirection::LeftToRight => padding[1] = padding[1].max(inset),
                LayoutDirection::RightToLeft => padding[0] = padding[0].max(inset),
            }
        }
        let has_description = self.description.is_some();
        let mut layout = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::StyledBox {
                style: BoxStyle {
                    width: self.width.map(Length::Points),
                    height: (!has_description)
                        .then(|| self.style.height.map(Length::Points))
                        .flatten(),
                    min_height: self.style.height.map(Length::Points),
                    max_width: self.style.max_width.map(Length::Points),
                    padding: Some(padding.map(Length::Points)),
                    alignment: BoxAlignment::Stretch,
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 1.0,
            }),
        )
        .composite(recipe_composite_style(&self.style));
        append_recipe_paint(&mut layout, cx, &self.style);
        layout.add_child(row_id);
        let layout_id = layout.build(cx);

        cx.pop_scope();
        cx.pop_scope();

        let mut actions = ActionSet::default();
        if !self.disabled {
            if let Some(action) = &self.on_select {
                actions.entries.push(ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: action.id.as_u128(),
                    payload_data: Some(action.payload.clone()),
                });
            }
        }
        let mut semantics = InternalIrBuilder::new(
            self.id,
            Op::Semantics(Semantics {
                role: self.role,
                identifier: self.semantics_identifier.clone(),
                label: Some(self.label.clone()),
                selected: self.selected,
                disabled: self.disabled,
                focusable: !self.disabled,
                sequential_focusable: !self.disabled && self.sequential_focusable,
                described_by: description_id.into_iter().collect(),
                actions,
                ..Default::default()
            }),
        );
        semantics.add_child(layout_id);
        semantics.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(WidgetId::derived(
            self.id.as_u128(),
            &[CUSTOM_WRAPPER_ID_PATH],
        ))
    }

    fn stable_key(&self) -> u64 {
        stable_id_key(self.id)
    }
}

fn append_recipe_paint(
    layout: &mut InternalIrBuilder,
    cx: &mut InternalLoweringCx,
    style: &ResolvedComponentStyle,
) {
    let radius = style.radius.unwrap_or(0.0);
    for shadow in &style.shadows {
        layout.add_child(
            InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: None,
                    stroke: None,
                    corner_radius: radius,
                    shadow: Some(shadow.to_box_shadow()),
                }),
            )
            .build(cx),
        );
    }
    let stroke = style.border.as_ref().map(|border| Stroke {
        fill: border.fill.clone(),
        width: border.width,
        dash_array: style.border_dash.clone(),
        line_cap: fission_core::op::LineCap::Butt,
        line_join: fission_core::op::LineJoin::Miter,
    });
    if style.background.is_some() || stroke.is_some() {
        layout.add_child(
            InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: style.background.clone(),
                    stroke,
                    corner_radius: radius,
                    shadow: None,
                }),
            )
            .build(cx),
        );
    }
}

fn recipe_composite_style(style: &ResolvedComponentStyle) -> CompositeStyle {
    CompositeStyle {
        opacity: style
            .opacity
            .filter(|opacity| opacity.is_finite() && *opacity != 1.0)
            .map(|opacity| CompositeScalar::new(opacity.clamp(0.0, 1.0))),
        translate_y: style
            .translate_y
            .filter(|offset| offset.is_finite() && *offset != 0.0)
            .map(CompositeScalar::new),
        ..Default::default()
    }
}

fn stable_id_key(id: WidgetId) -> u64 {
    let value = id.as_u128();
    value as u64 ^ (value >> 64) as u64
}

/// Retained trigger anatomy for a controlled menu button.
///
/// [`MenuButtonLayout`] supplies popup identity, expanded state, and the
/// toggle action. The standard presentation resolves its dimensions,
/// typography, spacing, and icon size from the active menu-trigger recipe. Use
/// [`Self::custom`] for richer retained trigger content while preserving an
/// explicit accessible label.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuTrigger {
    /// Localized or literal accessible trigger label.
    pub label: TextContent,
    /// Optional caller-provided retained presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Widget>,
    /// Design-system density used by the trigger.
    #[serde(default)]
    pub size: ComponentSize,
    /// Whether the trigger is visible but unavailable for interaction.
    #[serde(default)]
    pub disabled: bool,
    /// Stable identifier exposed on the trigger semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

impl MenuTrigger {
    /// Creates a standard enabled trigger.
    pub fn new(label: impl Into<TextContent>) -> Self {
        Self {
            label: label.into(),
            child: None,
            size: ComponentSize::Md,
            disabled: false,
            semantics_identifier: None,
        }
    }

    /// Creates a trigger with custom retained presentation and an explicit
    /// accessible label.
    pub fn custom(label: impl Into<TextContent>, child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::new(label)
        }
    }

    /// Uses a design-system density for trigger dimensions and typography.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Keeps the trigger visible while removing activation and focus behavior.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Exposes a stable identifier to accessibility and semantic tests.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

/// A complete controlled menu button assembled from retained named anatomy.
///
/// The layout owns the trigger/menu relationship, end-aligned anchoring,
/// dismissal, and focus containment. Its [`MenuContent`] may contain groups,
/// labels, separators, descriptions, leading content, and trailing metadata.
/// [`MenuButton`] remains the compact compatibility API and lowers through the
/// same recipe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuButtonLayout {
    /// Stable identity used for the trigger, popup, and portal.
    pub id: WidgetId,
    /// Retained trigger anatomy.
    pub trigger: MenuTrigger,
    /// Retained menu content.
    pub content: MenuContent,
    /// Controlled popup visibility.
    pub is_open: bool,
    /// Action requesting a controlled open-state change.
    pub on_toggle: Option<ActionEnvelope>,
}

impl MenuButtonLayout {
    /// Creates a closed controlled menu button.
    pub fn new(id: WidgetId, trigger: MenuTrigger, content: MenuContent) -> Self {
        Self {
            id,
            trigger,
            content,
            is_open: false,
            on_toggle: None,
        }
    }

    /// Sets the controlled open state.
    pub fn open(mut self, is_open: bool) -> Self {
        self.is_open = is_open;
        self
    }

    /// Sets the action used for trigger activation and popup dismissal.
    pub fn on_toggle(mut self, action: ActionEnvelope) -> Self {
        self.on_toggle = Some(action);
        self
    }
}

/// A button that toggles a [`Menu`] popover when clicked.
///
/// Renders an outline button with a label and a chevron icon. When `is_open`
/// is `true`, a flyout portal containing the menu items is displayed anchored
/// to the button. This source-compatible convenience API lowers through
/// [`MenuButtonLayout`], [`MenuTrigger`], and [`MenuContent`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MenuButton {
    /// Stable identity used to anchor and retain the flyout.
    pub id: WidgetId,
    /// Trigger-button label.
    pub label: String,
    /// Entries shown in the controlled menu.
    pub items: Vec<MenuItem>,
    /// Whether the controlled menu is currently open.
    pub is_open: bool,
    /// Action dispatched when the trigger requests an open-state change.
    pub on_toggle: Option<ActionEnvelope>,
    /// Stable identifier exposed on the generated trigger button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_semantics_identifier: Option<String>,
}

#[derive(Clone, Debug)]
struct MenuButtonRecipe {
    id: WidgetId,
    trigger: MenuTrigger,
    content: MenuContent,
    is_open: bool,
    on_toggle: Option<ActionEnvelope>,
}

impl From<MenuButton> for Widget {
    fn from(component: MenuButton) -> Self {
        let entries = component
            .items
            .into_iter()
            .map(MenuActionItem::from)
            .map(MenuEntry::Item)
            .collect();
        let trigger = MenuTrigger {
            semantics_identifier: component.trigger_semantics_identifier,
            ..MenuTrigger::new(component.label)
        };

        MenuButtonRecipe {
            id: component.id,
            trigger,
            content: MenuContent::new(entries).max_height(DEFAULT_MENU_MAX_HEIGHT),
            is_open: component.is_open,
            on_toggle: component.on_toggle,
        }
        .into()
    }
}

impl From<MenuButtonLayout> for Widget {
    fn from(component: MenuButtonLayout) -> Self {
        MenuButtonRecipe {
            id: component.id,
            trigger: component.trigger,
            content: component.content,
            is_open: component.is_open,
            on_toggle: component.on_toggle,
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct MenuTriggerRegion {
    id: WidgetId,
    popup_id: WidgetId,
    trigger: MenuTrigger,
    is_open: bool,
    on_toggle: Option<ActionEnvelope>,
}

impl From<MenuTriggerRegion> for Widget {
    fn from(region: MenuTriggerRegion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let theme = &view.env().theme.components.menu;
        let mut style = theme.resolve_trigger(region.trigger.size, ComponentState::Default);
        let mut padding = style.padding_box(tokens.spacing.s, tokens.spacing.xs);
        if view.env().layout_direction == LayoutDirection::RightToLeft {
            padding.swap(0, 1);
        }
        style.padding = Some(padding);
        let text_color = style.text_color.unwrap_or(tokens.colors.text_primary);
        let label = region.trigger.label.resolve(view.env());
        let child = region.trigger.child.unwrap_or_else(|| {
            HStack {
                spacing: Some(style.gap.unwrap_or(tokens.spacing.xs)),
                children: vec![
                    Text::new(region.trigger.label)
                        .size(style.font_size.unwrap_or(tokens.typography.font_size_base))
                        .weight(
                            style
                                .font_weight
                                .unwrap_or(tokens.typography.font_weight_medium),
                        )
                        .line_height(style.line_height.unwrap_or(20.0))
                        .color(text_color)
                        .into(),
                    Icon::svg(material::navigation::expand_more::regular())
                        .size(style.icon_size.unwrap_or(16.0))
                        .color(text_color)
                        .into(),
                ],
            }
            .into()
        });
        let trigger_semantics = Semantics {
            role: Role::Button,
            identifier: region.trigger.semantics_identifier,
            label: Some(label),
            expanded: Some(region.is_open),
            has_popup: Some(PopupKind::Menu),
            controls: vec![region.popup_id],
            focusable: !region.trigger.disabled,
            ..Default::default()
        };
        let style_override = ButtonStyleOverride {
            text_color: style.text_color,
            recipe_base: Some(style),
            state_styles: Some(theme.trigger_states.clone()),
            replace_component_recipe: true,
            ..Default::default()
        };

        Button {
            id: Some(region.id),
            variant: ButtonVariant::Outline,
            size: region.trigger.size,
            content_align: ButtonContentAlign::Start,
            child: Some(child),
            style: Some(style_override),
            on_press: (!region.trigger.disabled)
                .then_some(region.on_toggle)
                .flatten(),
            semantics: Some(trigger_semantics),
            disabled: region.trigger.disabled,
            ..Default::default()
        }
        .into()
    }
}

impl From<MenuButtonRecipe> for Widget {
    fn from(mut component: MenuButtonRecipe) -> Self {
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let trigger_id = WidgetId::derived(component.id.as_u128(), &[0, 1]);
        let popup_id = WidgetId::derived(component.id.as_u128(), POPUP_ID_PATH);
        let is_open = component.is_open && !component.trigger.disabled;
        let trigger: Widget = MenuTriggerRegion {
            id: trigger_id,
            popup_id,
            trigger: component.trigger,
            is_open,
            on_toggle: component.on_toggle.clone(),
        }
        .into();

        component.content.id = Some(popup_id);
        component.content.kind = MenuContentKind::Menu;
        component.content.on_dismiss = component.on_toggle.clone();
        component.content.focus_barrier = true;
        keep_focus_on_menu_button(&mut component.content.entries);
        let menu_content = component.content.into();

        popover_with_options(
            Popover {
                id: component.id,
                is_open,
                on_close: component.on_toggle,
                trigger,
                content: menu_content,
                motion: None,
            },
            FlyoutOptions::default()
                .with_alignment(FlyoutAlignment::End)
                .with_gap(4.0),
        )
    }
}

fn keep_focus_on_menu_button(entries: &mut [MenuEntry]) {
    for entry in entries {
        match entry {
            MenuEntry::Item(item) => {
                *item = item.clone().sequential_focusable(false);
            }
            MenuEntry::Group(group) => keep_focus_on_menu_button(&mut group.entries),
            MenuEntry::Label(_) | MenuEntry::Separator(_) => {}
        }
    }
}
