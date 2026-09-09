use crate::menu::menu_surface_item_id;
use crate::popover::{popover_with_options, Popover};
use crate::select::{
    SelectContent, SelectEntry, SelectGroup, SelectLabel, SelectOption, SelectSeparator,
};
use crate::{FlyoutOptions, FlyoutWidth, Icon};
use fission_core::ui::{TextInput, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use fission_ir::{PopupKind, Role};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DEFAULT_POPUP_MAX_HEIGHT: f32 = 240.0;
const INPUT_ID_PATH: &[u32] = &[0, 1];
const POPUP_ID_PATH: &[u32] = &[1];

/// Rich selectable-option anatomy shared with [`crate::Select`].
pub type ComboboxOption = SelectOption;

/// One option, group, label, or separator in [`ComboboxContent`].
pub type ComboboxEntry = SelectEntry;

/// A retained group of related combobox options.
pub type ComboboxGroup = SelectGroup;

/// A retained non-focusable label for a combobox group or region.
pub type ComboboxLabel = SelectLabel;

/// A retained non-focusable separator between combobox regions.
pub type ComboboxSeparator = SelectSeparator;

/// Retained editable-input anatomy for [`ComboboxLayout`].
///
/// The complete [`TextInput`] remains available so applications can configure
/// placeholders, leading content, validation, input policy, and platform text
/// behavior. The enclosing layout adds only the required combobox role, popup
/// relationships, default trailing indicator, and open-on-focus contract. An
/// explicitly configured [`TextInput::on_focus`] action remains authoritative;
/// in that case the application can request opening from the same reducer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComboboxInput {
    /// Editable control used as the popup trigger.
    pub input: TextInput,
    /// Whether focusing a closed field requests that the popup open.
    #[serde(default = "default_true")]
    pub open_on_focus: bool,
    /// Whether to add the standard trailing popup indicator when `input` does
    /// not already define a suffix.
    #[serde(default = "default_true")]
    pub show_indicator: bool,
}

const fn default_true() -> bool {
    true
}

impl ComboboxInput {
    /// Creates editable combobox input for a controlled value.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            input: TextInput {
                value: value.into(),
                ..Default::default()
            },
            open_on_focus: true,
            show_indicator: true,
        }
    }

    /// Uses a fully configured text input as the editable anatomy.
    pub fn custom(input: TextInput) -> Self {
        Self {
            input,
            open_on_focus: true,
            show_indicator: true,
        }
    }

    /// Controls whether focus requests the closed popup to open.
    pub fn open_on_focus(mut self, open_on_focus: bool) -> Self {
        self.open_on_focus = open_on_focus;
        self
    }

    /// Controls the default trailing popup indicator.
    pub fn show_indicator(mut self, show_indicator: bool) -> Self {
        self.show_indicator = show_indicator;
        self
    }
}

/// Retained list-box content for [`ComboboxLayout`].
///
/// Options share the select/menu list-box implementation, including groups,
/// labels, separators, descriptions, leading icons, metadata, selected
/// indicators, and disabled state.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComboboxContent {
    /// Options and structural anatomy in display order.
    pub entries: Vec<ComboboxEntry>,
    /// Optional logical popup width. The enclosing layout otherwise matches
    /// the editable field width.
    pub width: Option<f32>,
    /// Optional maximum logical height before options scroll.
    pub max_height: Option<f32>,
}

impl ComboboxContent {
    /// Creates list-box content from retained option anatomy.
    pub fn new(entries: Vec<ComboboxEntry>) -> Self {
        Self {
            entries,
            width: None,
            max_height: None,
        }
    }

    /// Uses an explicit logical popup width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Bounds the popup and enables vertical scrolling when needed.
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = Some(max_height);
        self
    }

    fn as_select_content(&self) -> SelectContent {
        SelectContent {
            entries: self.entries.clone(),
            width: self.width,
            max_height: self.max_height,
        }
    }

    fn into_select_content(self) -> SelectContent {
        SelectContent {
            entries: self.entries,
            width: self.width,
            max_height: self.max_height,
        }
    }
}

impl From<ComboboxContent> for Widget {
    fn from(content: ComboboxContent) -> Self {
        content.into_select_content().into()
    }
}

/// A complete controlled editable selection surface assembled from named anatomy.
///
/// This layout owns popup identity, editable combobox semantics, open-on-focus,
/// selected-option alignment, dismissal, and focus retention. The compact
/// [`Combobox`] API lowers through this same recipe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComboboxLayout {
    /// Stable identity used for the field, popup, and portal.
    pub id: WidgetId,
    /// Retained editable input anatomy.
    pub input: ComboboxInput,
    /// Retained popup anatomy.
    pub content: ComboboxContent,
    /// Controlled popup visibility.
    pub is_open: bool,
    /// Action requesting a controlled open-state change.
    pub on_toggle: Option<ActionEnvelope>,
    /// Optional logical width shared by field and popup when their anatomy
    /// does not set a more specific width.
    pub width: Option<f32>,
}

impl ComboboxLayout {
    /// Creates a closed editable selection surface.
    pub fn new(id: WidgetId, input: ComboboxInput, content: ComboboxContent) -> Self {
        Self {
            id,
            input,
            content,
            is_open: false,
            on_toggle: None,
            width: None,
        }
    }

    /// Sets the controlled open state.
    pub fn open(mut self, is_open: bool) -> Self {
        self.is_open = is_open;
        self
    }

    /// Sets the action used for focus opening and popup dismissal.
    pub fn on_toggle(mut self, action: ActionEnvelope) -> Self {
        self.on_toggle = Some(action);
        self
    }

    /// Supplies a shared fallback width for field and popup.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

/// A searchable dropdown combining a text input with a filterable option list.
///
/// The application owns the controlled value and supplies the currently
/// matching labels. This source-compatible convenience API lowers into
/// [`ComboboxLayout`], [`ComboboxInput`], [`ComboboxContent`], and the shared
/// retained list-box option anatomy. Use those named types directly when
/// options need descriptions, stable data identities, grouping, separators,
/// disabled state, or richer input configuration.
pub struct Combobox {
    /// Stable identity used for the editable field and popup anchor.
    pub id: WidgetId,
    /// Current controlled filter or selected text.
    pub value: String,
    /// Candidate labels presented in the popup.
    pub items: Vec<String>,
    /// Whether the controlled popup is open.
    pub is_open: bool,
    /// Optional logical width shared by field and popup.
    pub width: Option<f32>,
    /// Optional maximum popup height before scrolling.
    pub max_popup_height: Option<f32>,
    /// Action dispatched for edits; text details arrive through `ActionInput`.
    pub on_input: Option<ActionEnvelope>,
    /// Factory producing an action when an item is selected.
    pub on_select: Option<Arc<dyn Fn(String) -> ActionEnvelope + Send + Sync>>,
    /// Action requesting a popup open-state change.
    pub on_toggle: Option<ActionEnvelope>,
}

impl std::fmt::Debug for Combobox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Combobox")
            .field("value", &self.value)
            .field("items_count", &self.items.len())
            .finish()
    }
}

#[derive(Clone, Debug)]
struct ComboboxRecipe {
    id: WidgetId,
    input: ComboboxInput,
    content: ComboboxContent,
    is_open: bool,
    on_toggle: Option<ActionEnvelope>,
    width: Option<f32>,
    align_selected: bool,
}

impl From<Combobox> for Widget {
    fn from(component: Combobox) -> Self {
        let unambiguous_value = component
            .items
            .iter()
            .filter(|item| item.as_str() == component.value.as_str())
            .take(2)
            .count()
            == 1;
        let mut selected_found = false;
        let entries = component
            .items
            .into_iter()
            .map(|item| {
                let selected = !selected_found && item == component.value;
                selected_found |= selected;
                let mut option =
                    ComboboxOption::option(item.clone(), selected).sequential_focusable(false);
                option.on_select = component.on_select.clone().map(|callback| callback(item));
                ComboboxEntry::Item(option)
            })
            .collect();
        let mut input = ComboboxInput::new(component.value);
        input.input.on_input = component.on_input;
        input.input.width = component.width;

        ComboboxRecipe {
            id: component.id,
            input,
            content: ComboboxContent::new(entries).max_height(
                component
                    .max_popup_height
                    .unwrap_or(DEFAULT_POPUP_MAX_HEIGHT),
            ),
            is_open: component.is_open,
            on_toggle: component.on_toggle,
            width: component.width,
            align_selected: unambiguous_value,
        }
        .into()
    }
}

impl From<ComboboxLayout> for Widget {
    fn from(component: ComboboxLayout) -> Self {
        ComboboxRecipe {
            id: component.id,
            input: component.input,
            content: component.content,
            is_open: component.is_open,
            on_toggle: component.on_toggle,
            width: component.width,
            align_selected: true,
        }
        .into()
    }
}

impl From<ComboboxRecipe> for Widget {
    fn from(mut component: ComboboxRecipe) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let popup_id = WidgetId::derived(component.id.as_u128(), POPUP_ID_PATH);
        let input_id = component
            .input
            .input
            .id
            .unwrap_or_else(|| WidgetId::derived(component.id.as_u128(), INPUT_ID_PATH));
        let select_content = component.content.as_select_content();
        let selected = select_content.selected_option_ids(popup_id);
        let selected_descendant = (selected.len() == 1).then(|| selected[0]);
        let mut enabled_options = Vec::new();
        let mut option_index = 0;
        collect_enabled_option_ids(
            &select_content.entries,
            popup_id,
            &mut option_index,
            &mut enabled_options,
        );
        let runtime_active = view
            .runtime()
            .interaction
            .active_descendant(input_id)
            .filter(|candidate| enabled_options.contains(candidate));
        let active_descendant = runtime_active.or(selected_descendant);
        let is_open =
            component.is_open && component.input.input.enabled && select_content.has_options();

        let mut input = component.input.input;
        input.id = Some(input_id);
        input.width = input.width.or(component.width);
        let mut semantics = input.semantics.take().unwrap_or_default();
        semantics.role = Role::ComboBox;
        semantics.expanded = Some(is_open);
        semantics.has_popup = Some(PopupKind::ListBox);
        semantics.controls = vec![popup_id];
        semantics.active_descendant = is_open.then_some(active_descendant).flatten();
        input.semantics = Some(semantics);
        if component.input.open_on_focus && input.on_focus.is_none() {
            input.on_focus = (!component.is_open)
                .then(|| component.on_toggle.clone())
                .flatten();
        }
        if component.input.show_indicator && input.suffix.is_none() {
            let popup_icon = if is_open {
                material::navigation::expand_less::regular()
            } else {
                material::navigation::expand_more::regular()
            };
            input.suffix = Some(
                Icon::svg(popup_icon)
                    .size(view.env().theme.components.button.icon_size)
                    .color(view.env().theme.tokens.colors.text_secondary)
                    .into(),
            );
        }

        let content = component.content.into_select_content().into_menu_content(
            popup_id,
            component.width,
            component.on_toggle.clone(),
            false,
        );
        let mut flyout_options = FlyoutOptions::default()
            .with_width(FlyoutWidth::MatchAnchor)
            .with_gap(4.0);
        if let Some(selected_id) =
            selected_descendant.filter(|_| is_open && component.align_selected)
        {
            flyout_options = flyout_options.with_alignment_target(selected_id);
        }

        popover_with_options(
            Popover {
                id: component.id,
                is_open,
                on_close: component.on_toggle,
                trigger: input.into(),
                content: content.into(),
                motion: None,
            },
            flyout_options,
        )
    }
}

fn collect_enabled_option_ids(
    entries: &[SelectEntry],
    popup_id: WidgetId,
    item_index: &mut usize,
    enabled: &mut Vec<WidgetId>,
) {
    for entry in entries {
        match entry {
            SelectEntry::Item(item) => {
                let id = item.id.unwrap_or_else(|| {
                    menu_surface_item_id(
                        popup_id,
                        *item_index,
                        item.semantics_identifier.as_deref(),
                    )
                });
                if !item.disabled {
                    enabled.push(id);
                }
                *item_index += 1;
            }
            SelectEntry::Group(group) => {
                collect_enabled_option_ids(&group.entries, popup_id, item_index, enabled)
            }
            SelectEntry::Label(_) | SelectEntry::Separator(_) => {}
        }
    }
}
