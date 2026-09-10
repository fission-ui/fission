use crate::menu::{
    menu_surface_item_id, MenuActionItem, MenuContent, MenuEntry, MenuGroup, MenuLabel,
    MenuSeparator,
};
use crate::popover::{popover_with_options, Popover};
use crate::stack::HStack;
use crate::{FlyoutOptions, FlyoutWidth, Icon};
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonStyleOverride, ButtonVariant, ComponentSize, Spacer, Text,
    TextContent, Widget,
};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use fission_ir::{LayoutDirection, PopupKind, Role, Semantics, TextFieldValidationState};
use fission_theme::ComponentState;
use serde::{Deserialize, Serialize};

const POPUP_ID_PATH: &[u32] = &[1];
const IMPLICIT_SELECT_CONTENT_ID_SALT: u32 = 0x5343_4e54;

/// Rich option anatomy shared with menu-backed list boxes.
///
/// Construct options with [`SelectOption::option`]. Supporting descriptions,
/// leading icons, metadata, stable identity, disabled state, and selection are
/// all retained by the shared menu/list-box implementation.
pub type SelectOption = MenuActionItem;

/// One option, group, label, or separator in [`SelectContent`].
pub type SelectEntry = MenuEntry;

/// A retained group of related select entries.
pub type SelectGroup = MenuGroup;

/// A retained non-focusable label for a select group or region.
pub type SelectLabel = MenuLabel;

/// A retained non-focusable separator between select regions.
pub type SelectSeparator = MenuSeparator;

/// A single compatibility option in a [`Select`] dropdown.
///
/// Use [`SelectOption`] and [`SelectContent`] when an option needs supporting
/// text, disabled state, metadata, grouping, or explicit separators.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectItem {
    /// Text displayed for the option.
    pub label: String,
    /// Optional SVG markup displayed before the label.
    pub icon: Option<String>,
    /// Action dispatched when the option is selected.
    pub on_select: ActionEnvelope,
    /// Stable identifier exposed on this option's actionable semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

/// Retained trigger anatomy for a read-only selection control.
///
/// [`SelectLayout`] supplies popup identity, expanded state, and the toggle
/// action. The standard presentation resolves typography, spacing, control
/// density, and icon size from the active select-trigger recipe. Use [`Self::custom`]
/// for richer retained value presentation while preserving the accessible
/// value and complete combobox semantics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectTrigger {
    /// Localized or literal value exposed visually and through semantics.
    pub value: TextContent,
    /// Optional caller-provided retained presentation for `value`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Widget>,
    /// Whether the standard value presentation uses placeholder treatment.
    pub placeholder: bool,
    /// Design-system density used by the trigger.
    #[serde(default)]
    pub size: ComponentSize,
    /// Whether the trigger is visible but unavailable for interaction.
    #[serde(default)]
    pub disabled: bool,
    /// Whether the controlled selection is invalid.
    #[serde(default)]
    pub invalid: bool,
    /// Stable identifier exposed on the trigger semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

impl SelectTrigger {
    /// Creates a standard selected-value trigger.
    pub fn new(value: impl Into<TextContent>) -> Self {
        Self {
            value: value.into(),
            child: None,
            placeholder: false,
            size: ComponentSize::Md,
            disabled: false,
            invalid: false,
            semantics_identifier: None,
        }
    }

    /// Creates a standard trigger using subdued placeholder presentation.
    pub fn placeholder(value: impl Into<TextContent>) -> Self {
        Self {
            placeholder: true,
            ..Self::new(value)
        }
    }

    /// Creates a trigger with custom retained presentation and an explicit
    /// accessible value.
    pub fn custom(value: impl Into<TextContent>, child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::new(value)
        }
    }

    /// Uses a design-system density for trigger dimensions and typography.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Keeps the value visible while removing activation and focus behavior.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the design-system error recipe and invalid field semantics.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Exposes a stable identifier to accessibility and semantic tests.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

/// Retained list-box content for [`SelectLayout`].
///
/// Entries share the same implementation authority as menu anatomy but are
/// normalized to list-box and option semantics. This supports groups, labels,
/// separators, descriptions, leading icons, metadata, selected indicators,
/// and disabled options without applications rebuilding popup rows.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectContent {
    /// Options and structural anatomy in display order.
    pub entries: Vec<SelectEntry>,
    /// Optional logical popup width. The enclosing layout otherwise matches
    /// the trigger width.
    pub width: Option<f32>,
    /// Optional maximum logical height before the options scroll.
    pub max_height: Option<f32>,
}

impl SelectContent {
    /// Creates list-box content from retained option anatomy.
    pub fn new(entries: Vec<SelectEntry>) -> Self {
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

    pub(crate) fn selected_option_ids(&self, popup_id: WidgetId) -> Vec<WidgetId> {
        let mut item_index = 0;
        let mut selected = Vec::new();
        collect_selected_option_ids(&self.entries, popup_id, &mut item_index, &mut selected);
        selected
    }

    pub(crate) fn has_options(&self) -> bool {
        entries_have_options(&self.entries)
    }

    pub(crate) fn into_menu_content(
        mut self,
        popup_id: WidgetId,
        width: Option<f32>,
        dismiss_action: Option<ActionEnvelope>,
        focus_barrier: bool,
    ) -> MenuContent {
        keep_focus_on_composite(&mut self.entries);
        let mut content = MenuContent::list_box(self.entries)
            .id(popup_id)
            .dismiss_action(dismiss_action)
            .focus_barrier(focus_barrier);
        content.width = self.width.or(width);
        content.max_height = self.max_height;
        content
    }
}

impl From<SelectContent> for Widget {
    fn from(content: SelectContent) -> Self {
        let popup_id = fission_core::build::current_widget_id()
            .or_else(|| {
                fission_core::build::next_implicit_widget_id(IMPLICIT_SELECT_CONTENT_ID_SALT)
            })
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.select.content"));
        content
            .into_menu_content(popup_id, None, None, false)
            .into()
    }
}

/// A complete controlled selection surface assembled from named anatomy.
///
/// This layout owns the relationship between its trigger and popup, including
/// stable identities, selected-option alignment, dismissal, and the sole
/// activation action. [`Select`] remains the compact compatibility API and
/// lowers through this same recipe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectLayout {
    /// Stable identity used for the trigger, popup, and portal.
    pub id: WidgetId,
    /// Retained trigger anatomy.
    pub trigger: SelectTrigger,
    /// Retained popup anatomy.
    pub content: SelectContent,
    /// Controlled popup visibility.
    pub is_open: bool,
    /// Action requesting a controlled open-state change.
    pub on_toggle: Option<ActionEnvelope>,
    /// Optional logical width shared by trigger and popup.
    pub width: Option<f32>,
}

impl SelectLayout {
    /// Creates a closed controlled selection surface.
    pub fn new(id: WidgetId, trigger: SelectTrigger, content: SelectContent) -> Self {
        Self {
            id,
            trigger,
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

    /// Sets the action used for trigger activation and popup dismissal.
    pub fn on_toggle(mut self, action: ActionEnvelope) -> Self {
        self.on_toggle = Some(action);
        self
    }

    /// Uses one logical width for the standard trigger and popup.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

/// A dropdown selector that displays the selected label and opens a list-box flyout.
///
/// This source-compatible convenience API lowers into [`SelectLayout`],
/// [`SelectTrigger`], [`SelectContent`], and the shared retained option anatomy.
/// Use those named types directly for groups, labels, separators, supporting
/// descriptions, disabled options, metadata, or custom retained trigger content.
///
/// # Example
///
/// ```rust,ignore
/// Select {
///     id: WidgetId::explicit("country"),
///     selected_label: Some("United States".into()),
///     items: vec![
///         SelectItem { label: "United States".into(), icon: None, on_select: us_action, semantics_identifier: Some("country.us".into()) },
///         SelectItem { label: "Canada".into(), icon: None, on_select: ca_action, semantics_identifier: Some("country.ca".into()) },
///     ],
///     is_open: state.country_open,
///     on_toggle: Some(toggle_action),
///     trigger_semantics_identifier: Some("country.open".into()),
///     placeholder: "Choose country...".into(),
///     width: Some(250.0),
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Select {
    /// Stable identity used to anchor and retain the popup surface.
    pub id: WidgetId,
    /// Current controlled selection label, or `None` to show `placeholder`.
    pub selected_label: Option<String>,
    /// Options presented in the popup menu.
    pub items: Vec<SelectItem>,
    /// Whether the controlled popup is currently open.
    pub is_open: bool,
    /// Action dispatched when the trigger requests an open-state change.
    pub on_toggle: Option<ActionEnvelope>,
    /// Stable identifier exposed on the generated trigger button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_semantics_identifier: Option<String>,
    /// Text shown when no selection is present.
    pub placeholder: String,
    /// Optional logical width shared by the trigger and popup.
    pub width: Option<f32>,
}

impl Default for Select {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("select"),
            selected_label: None,
            items: Vec::new(),
            is_open: false,
            on_toggle: None,
            trigger_semantics_identifier: None,
            placeholder: "Select...".into(),
            width: Some(200.0),
        }
    }
}

#[derive(Clone, Debug)]
struct SelectRecipe {
    id: WidgetId,
    trigger: SelectTrigger,
    content: SelectContent,
    is_open: bool,
    on_toggle: Option<ActionEnvelope>,
    width: Option<f32>,
}

impl From<Select> for Widget {
    fn from(component: Select) -> Self {
        let selected_label = component.selected_label.as_deref();
        let mut selected_found = false;
        let entries = component
            .items
            .into_iter()
            .map(|item| {
                let selected = !selected_found && selected_label == Some(item.label.as_str());
                selected_found |= selected;
                let mut option = SelectOption::option(item.label, selected)
                    .on_select(item.on_select)
                    .sequential_focusable(false);
                option.leading_icon = item.icon;
                option.semantics_identifier = item.semantics_identifier;
                SelectEntry::Item(option)
            })
            .collect();
        let trigger = match component.selected_label {
            Some(label) => SelectTrigger::new(label),
            None => SelectTrigger::placeholder(component.placeholder),
        };
        let trigger = SelectTrigger {
            semantics_identifier: component.trigger_semantics_identifier,
            ..trigger
        };

        SelectRecipe {
            id: component.id,
            trigger,
            content: SelectContent::new(entries),
            is_open: component.is_open,
            on_toggle: component.on_toggle,
            width: component.width,
        }
        .into()
    }
}

impl From<SelectLayout> for Widget {
    fn from(component: SelectLayout) -> Self {
        SelectRecipe {
            id: component.id,
            trigger: component.trigger,
            content: component.content,
            is_open: component.is_open,
            on_toggle: component.on_toggle,
            width: component.width,
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct SelectTriggerRegion {
    id: WidgetId,
    popup_id: WidgetId,
    trigger: SelectTrigger,
    is_open: bool,
    on_toggle: Option<ActionEnvelope>,
    active_descendant: Option<WidgetId>,
    width: Option<f32>,
}

impl From<SelectTriggerRegion> for Widget {
    fn from(region: SelectTriggerRegion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let theme = &view.env().theme.components.select;
        let mut style = theme.resolve_trigger(region.trigger.size, ComponentState::Default);
        let value_style = if region.trigger.placeholder {
            style.merge(&theme.placeholder_style)
        } else {
            style.clone()
        };
        let mut padding = style.padding_box(tokens.spacing.s, tokens.spacing.xs);
        let indicator_inset = theme.indicator_style.inset_end.unwrap_or(tokens.spacing.s);
        match view.env().layout_direction {
            LayoutDirection::LeftToRight => padding[1] = padding[1].max(indicator_inset),
            LayoutDirection::RightToLeft => {
                padding.swap(0, 1);
                padding[0] = padding[0].max(indicator_inset);
            }
        }
        style.padding = Some(padding);
        let resolved_value = region.trigger.value.resolve(view.env());
        let child = region.trigger.child.unwrap_or_else(|| {
            HStack {
                spacing: Some(style.gap.unwrap_or(tokens.spacing.xs)),
                children: vec![
                    Text::new(region.trigger.value)
                        .size(
                            value_style
                                .font_size
                                .unwrap_or(tokens.typography.font_size_base),
                        )
                        .weight(
                            value_style
                                .font_weight
                                .unwrap_or(tokens.typography.font_weight_regular),
                        )
                        .line_height(value_style.line_height.unwrap_or(20.0))
                        .color(value_style.text_color.unwrap_or(tokens.colors.text_primary))
                        .into(),
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into(),
                    Icon::svg(material::navigation::expand_more::regular())
                        .size(theme.indicator_style.icon_size.unwrap_or(16.0))
                        .color(
                            theme
                                .indicator_style
                                .text_color
                                .unwrap_or(tokens.colors.text_secondary),
                        )
                        .into(),
                ],
            }
            .into()
        });
        let semantics = Semantics {
            role: Role::ComboBox,
            identifier: region.trigger.semantics_identifier,
            value: Some(resolved_value),
            expanded: Some(region.is_open),
            has_popup: Some(PopupKind::ListBox),
            controls: vec![region.popup_id],
            active_descendant: region.is_open.then_some(region.active_descendant).flatten(),
            focusable: !region.trigger.disabled,
            validation_state: if region.trigger.invalid {
                TextFieldValidationState::Invalid
            } else {
                TextFieldValidationState::Unvalidated
            },
            ..Default::default()
        };
        let style_override = ButtonStyleOverride {
            text_color: value_style.text_color,
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
            semantics: Some(semantics),
            width: region.width,
            disabled: region.trigger.disabled,
            ..Default::default()
        }
        .into()
    }
}

impl From<SelectRecipe> for Widget {
    fn from(mut component: SelectRecipe) -> Self {
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let popup_id = WidgetId::derived(component.id.as_u128(), POPUP_ID_PATH);
        let trigger_id = WidgetId::derived(component.id.as_u128(), &[0, 1]);
        let selected = component.content.selected_option_ids(popup_id);
        let active_descendant = (selected.len() == 1).then(|| selected[0]);
        let is_open = component.is_open && !component.trigger.disabled;
        let trigger: Widget = SelectTriggerRegion {
            id: trigger_id,
            popup_id,
            trigger: component.trigger,
            is_open,
            on_toggle: component.on_toggle.clone(),
            active_descendant,
            width: component.width,
        }
        .into();
        let content = component.content.into_menu_content(
            popup_id,
            component.width,
            component.on_toggle.clone(),
            true,
        );
        let mut flyout_options = FlyoutOptions::default()
            .with_width(FlyoutWidth::MatchAnchor)
            .with_gap(4.0);
        if let Some(active_descendant) = active_descendant.filter(|_| is_open) {
            flyout_options = flyout_options.with_alignment_target(active_descendant);
        }

        popover_with_options(
            Popover {
                id: component.id,
                is_open,
                on_close: component.on_toggle,
                trigger,
                content: content.into(),
                motion: None,
            },
            flyout_options,
        )
    }
}

fn collect_selected_option_ids(
    entries: &[SelectEntry],
    popup_id: WidgetId,
    item_index: &mut usize,
    selected: &mut Vec<WidgetId>,
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
                if item.selected == Some(true) && !item.disabled {
                    selected.push(id);
                }
                *item_index += 1;
            }
            SelectEntry::Group(group) => {
                collect_selected_option_ids(&group.entries, popup_id, item_index, selected)
            }
            SelectEntry::Label(_) | SelectEntry::Separator(_) => {}
        }
    }
}

fn entries_have_options(entries: &[SelectEntry]) -> bool {
    entries.iter().any(|entry| match entry {
        SelectEntry::Item(_) => true,
        SelectEntry::Group(group) => entries_have_options(&group.entries),
        SelectEntry::Label(_) | SelectEntry::Separator(_) => false,
    })
}

fn keep_focus_on_composite(entries: &mut [SelectEntry]) {
    for entry in entries {
        match entry {
            SelectEntry::Item(item) => {
                *item = item.clone().sequential_focusable(false);
            }
            SelectEntry::Group(group) => keep_focus_on_composite(&mut group.entries),
            SelectEntry::Label(_) | SelectEntry::Separator(_) => {}
        }
    }
}
