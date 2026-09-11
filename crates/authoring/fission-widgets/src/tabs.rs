use crate::motion_support::{
    dedupe, exit_for, fade_in, push_enter_with_exit, slide_x_in, slot_id, SLOT_CONTENT,
    SLOT_INDICATOR,
};
use crate::stack::VStack;
use crate::Badge;
use fission_core::authoring::{IrBuilder, LowerWidget, LoweringCx};
use fission_core::motion::{follow_x_and_width, Motion, MotionTrack, Presence};
use fission_core::op::{
    AlignItems, BoxAlignment, BoxStyle, Fill, FlexDirection, FlexWrap, JustifyContent, LayoutOp,
    Length, Op, PaintOp, Stroke,
};
use fission_core::ui::{
    ComponentSize, ComponentState, Composite, Container, Icon, Row, Scroll, Text, TextContent,
    Widget,
};
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::semantics::{ActionTrigger, SemanticOrientation};
use fission_ir::{ActionEntry, ActionSet, CompositeScalar, CompositeStyle, Role, Semantics};
use fission_theme::ResolvedComponentStyle;
pub use fission_theme::TabPresentation;
use serde::{Deserialize, Serialize};
use std::ops::Add;

const TAB_LIST_ID_SALT: u32 = 0x5441_424c;
const TAB_PANEL_ID_SALT: u32 = 0x5041_4e4c;
const CUSTOM_WRAPPER_ID_SALT: u32 = 0x5752_4150;
const IMPLICIT_TABS_ID_SALT: u32 = 0x5441_4253;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Tabs`].
///
/// Tabs render immediately unless [`Tabs::motion`] is set. Presets lower to
/// motion tracks for the stable `indicator` slot and presence tracks for the
/// active `content` slot.
///
/// ```rust,ignore
/// let motion = Some(TabsMotion::Indicator + TabsMotion::SlideContent);
/// ```
pub enum TabsMotion {
    /// Curated default: indicator plus fading content.
    Default,
    /// Animate the active indicator toward the active tab trigger.
    Indicator,
    /// Fade tab content.
    FadeContent,
    /// Slide tab content.
    SlideContent,
    /// Compound indicator plus fade content preset.
    IndicatorAndFade,
    /// Compound indicator plus slide content preset.
    IndicatorAndSlide,
    /// Ordered composition of tabs motion atoms.
    Composition(Vec<TabsMotion>),
    /// Caller-provided tracks for tabs stable slots.
    Custom {
        /// Optional indicator tracks.
        indicator: Option<Vec<MotionTrack>>,
        /// Content enter tracks.
        content_enter: Vec<MotionTrack>,
        /// Content exit tracks.
        content_exit: Vec<MotionTrack>,
        /// Whether content remains rendered after exit completes.
        keep_rendered: bool,
    },
}

impl TabsMotion {
    /// Flattens and normalizes an ordered tabs-motion composition.
    pub fn compose(items: impl IntoIterator<Item = Self>) -> Self {
        let mut out = Vec::new();
        for item in items {
            item.flatten_into(&mut out);
        }
        match out.len() {
            0 => Self::Composition(Vec::new()),
            1 => out.remove(0),
            _ => Self::Composition(out),
        }
    }

    fn flatten_into(self, out: &mut Vec<Self>) {
        match self {
            Self::Composition(items) => {
                for item in items {
                    item.flatten_into(out);
                }
            }
            item => out.push(item),
        }
    }

    fn plan(&self, active_trigger: WidgetId) -> TabsMotionPlan {
        let mut plan = TabsMotionPlan::default();
        self.append_plan(active_trigger, &mut plan);
        plan.normalize()
    }

    fn append_plan(&self, active_trigger: WidgetId, plan: &mut TabsMotionPlan) {
        match self {
            Self::Default | Self::IndicatorAndFade => {
                Self::Indicator.append_plan(active_trigger, plan);
                Self::FadeContent.append_plan(active_trigger, plan);
            }
            Self::IndicatorAndSlide => {
                Self::Indicator.append_plan(active_trigger, plan);
                Self::SlideContent.append_plan(active_trigger, plan);
            }
            Self::Indicator => plan.indicator.extend(follow_x_and_width(active_trigger)),
            Self::FadeContent => {
                push_enter_with_exit(
                    &mut plan.content_enter,
                    &mut plan.content_exit,
                    fade_in(140),
                );
            }
            Self::SlideContent => {
                push_enter_with_exit(
                    &mut plan.content_enter,
                    &mut plan.content_exit,
                    slide_x_in(12.0, 160),
                );
            }
            Self::Composition(items) => {
                for item in items {
                    item.append_plan(active_trigger, plan);
                }
            }
            Self::Custom {
                indicator,
                content_enter,
                content_exit,
                keep_rendered,
            } => {
                if let Some(indicator) = indicator {
                    plan.indicator.extend(indicator.clone());
                }
                plan.content_enter.extend(content_enter.clone());
                plan.content_exit.extend(content_exit.clone());
                plan.keep_rendered |= *keep_rendered;
            }
        }
    }
}

impl Add for TabsMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

#[derive(Default)]
struct TabsMotionPlan {
    indicator: Vec<MotionTrack>,
    content_enter: Vec<MotionTrack>,
    content_exit: Vec<MotionTrack>,
    keep_rendered: bool,
}

impl TabsMotionPlan {
    fn normalize(mut self) -> Self {
        if self.content_exit.is_empty() {
            self.content_exit = exit_for(&self.content_enter);
        }
        self.indicator = dedupe(self.indicator);
        self.content_enter = dedupe(self.content_enter);
        self.content_exit = dedupe(self.content_exit);
        self
    }
}

/// A single tab definition containing a title, content node, and selection action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabItem {
    /// Label displayed in the tab bar.
    pub title: String,
    /// Content shown when this item is selected.
    pub content: Widget,
    /// Action dispatched when this tab's trigger is pressed.
    pub on_press: Option<ActionEnvelope>,
    /// Stable identifier exposed on this tab's actionable semantics node.
    ///
    /// Use this to address tabs independently when visible titles are
    /// duplicated or localized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
}

/// A themed tab list with a selected trigger and swappable content panel.
///
/// The tab list displays a horizontal row of tab triggers. Themes may use a
/// segmented selected surface, an underline, or both. The content area below
/// the tab list displays the `content` node of the tab at `active_index`.
///
/// # Example
///
/// ```rust,ignore
/// Tabs {
///     active_index: 0,
///     items: vec![
///         TabItem { title: "General".into(), content: general_view, on_press: Some(tab0), semantics_identifier: Some("settings.general".into()) },
///         TabItem { title: "Advanced".into(), content: advanced_view, on_press: Some(tab1), semantics_identifier: Some("settings.advanced".into()) },
///     ],
/// }
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Tabs {
    /// Zero-based selected item index; an out-of-range value resolves to the first item.
    pub active_index: usize,
    /// Tab definitions in display order.
    pub items: Vec<TabItem>,
    /// Design-system size used for tab typography, spacing, and indicator style.
    pub size: ComponentSize,
    /// Optional explicit tabs motion. `None` emits no tabs-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<TabsMotion>,
}

/// Retained trigger anatomy for one tab.
///
/// The trigger owns its tab role, selected state, controlled action, panel
/// relationship, roving-focus state, and design-system presentation. Use
/// [`Self::custom`] for icon, badge, or other retained label content while
/// preserving an explicit accessible label.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabTrigger {
    /// Stable identity used for focus, interaction state, and relationships.
    pub id: WidgetId,
    /// Stable identity of the panel controlled by this trigger.
    pub panel_id: WidgetId,
    /// Localized or literal accessible label.
    pub label: TextContent,
    /// Optional custom retained label presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Widget>,
    /// Action dispatched when this tab is selected.
    pub on_press: Option<ActionEnvelope>,
    /// Whether this trigger requests ownership of the active panel.
    ///
    /// Selection is controlled by the application. [`TabList`] resolves
    /// malformed input to one rendered selection without dispatching an action
    /// or mutating application state.
    pub selected: bool,
    /// Whether this trigger remains visible but cannot be focused or selected.
    #[serde(default)]
    pub disabled: bool,
    /// Design-system density used for trigger presentation.
    #[serde(default)]
    pub size: ComponentSize,
    /// Stable identifier exposed to shells and semantic tests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
    #[serde(default)]
    presentation: TabPresentation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<TabTriggerContent>,
    #[serde(skip)]
    indicator_tracks: Vec<MotionTrack>,
    #[serde(skip)]
    indicator_slot: Option<WidgetId>,
    #[serde(skip)]
    sequential_focusable: Option<bool>,
}

/// Recipe-aware visible content for one semantic tab trigger.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabTriggerContent {
    pub label: TextContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leading_icon: Option<Icon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing_count: Option<String>,
}

impl TabTriggerContent {
    pub fn new(label: impl Into<TextContent>) -> Self {
        Self {
            label: label.into(),
            leading_icon: None,
            trailing_count: None,
        }
    }

    pub fn leading_icon(mut self, icon: Icon) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    pub fn trailing_count(mut self, count: impl Into<String>) -> Self {
        self.trailing_count = Some(count.into());
        self
    }
}

impl TabTrigger {
    /// Creates a standard unselected tab trigger.
    pub fn new(id: WidgetId, panel_id: WidgetId, label: impl Into<TextContent>) -> Self {
        Self {
            id,
            panel_id,
            label: label.into(),
            child: None,
            on_press: None,
            selected: false,
            disabled: false,
            size: ComponentSize::Md,
            semantics_identifier: None,
            presentation: TabPresentation::Default,
            content: None,
            indicator_tracks: Vec::new(),
            indicator_slot: None,
            sequential_focusable: None,
        }
    }

    /// Creates a trigger with custom retained presentation and an explicit
    /// accessible label.
    pub fn custom(
        id: WidgetId,
        panel_id: WidgetId,
        label: impl Into<TextContent>,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::new(id, panel_id, label)
        }
    }

    /// Sets the controlled selected state.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Keeps the tab visible while excluding it from activation and focus navigation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Dispatches `action` when the trigger is activated.
    pub fn on_press(mut self, action: ActionEnvelope) -> Self {
        self.on_press = Some(action);
        self
    }

    /// Uses a design-system density for the trigger.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Exposes a stable identifier to accessibility and semantic tests.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    /// Uses recipe-aware label, icon, and count anatomy.
    pub fn content(mut self, content: TabTriggerContent) -> Self {
        self.label = content.label.clone();
        self.child = None;
        self.content = Some(content);
        self
    }

    /// Selects a named design-system treatment for this trigger.
    pub fn presentation(mut self, presentation: TabPresentation) -> Self {
        self.presentation = presentation;
        self
    }

    fn indicator_tracks(mut self, slot: WidgetId, tracks: Vec<MotionTrack>) -> Self {
        self.indicator_slot = Some(slot);
        self.indicator_tracks = tracks;
        self
    }
}

impl From<TabTrigger> for Widget {
    fn from(trigger: TabTrigger) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.tabs;
        let interaction = &view.runtime().interaction;
        let is_pressed = interaction.is_pressed(trigger.id);
        let is_focused = interaction.is_focused(trigger.id);
        let selected = trigger.selected && !trigger.disabled;
        let state = if trigger.disabled {
            ComponentState::Disabled
        } else if selected || is_pressed {
            ComponentState::Active
        } else if is_focused {
            ComponentState::Focus
        } else if interaction.is_hovered(trigger.id) {
            ComponentState::Hover
        } else {
            ComponentState::Default
        };
        let mut style = theme.resolve_tab_presentation(trigger.presentation, trigger.size, state);
        if selected && is_focused {
            if let Some(focus) = &theme.states.focus {
                style = style.merge_composing_shadows(focus);
            }
        }
        let text_color = style.text_color.unwrap_or(if selected || is_pressed {
            theme.active_color
        } else {
            theme.inactive_color
        });
        let label = trigger.label.resolve(view.env());
        let child = trigger
            .content
            .as_ref()
            .map(|content| {
                let mut children = Vec::with_capacity(3);
                if let Some(mut icon) = content.leading_icon.clone() {
                    icon.color.get_or_insert(text_color);
                    icon.size.get_or_insert(style.icon_size.unwrap_or(16.0));
                    children.push(icon.into());
                }
                let mut text = Text::new(content.label.clone())
                    .size(style.font_size.unwrap_or(14.0))
                    .weight(style.font_weight.unwrap_or(400))
                    .color(text_color);
                if let Some(line_height) = style.line_height {
                    text = text.line_height(line_height);
                }
                if let Some(letter_spacing) = style.letter_spacing {
                    text = text.letter_spacing(letter_spacing);
                }
                children.push(text.into());
                if let Some(count) = content.trailing_count.clone() {
                    children.push(
                        Badge {
                            text: count,
                            size: trigger.size,
                            ..Default::default()
                        }
                        .into(),
                    );
                }
                Row {
                    gap: style.gap,
                    align_items: AlignItems::Center,
                    children,
                    ..Default::default()
                }
                .into()
            })
            .unwrap_or_else(|| {
                trigger.child.clone().unwrap_or_else(|| {
                    let mut text = Text::new(trigger.label.clone())
                        .size(style.font_size.unwrap_or(14.0))
                        .weight(style.font_weight.unwrap_or(400))
                        .color(text_color);
                    if let Some(line_height) = style.line_height {
                        text = text.line_height(line_height);
                    }
                    if let Some(letter_spacing) = style.letter_spacing {
                        text = text.letter_spacing(letter_spacing);
                    }
                    text.into()
                })
            });
        let indicator_fill = Fill::Solid(theme.active_color);
        let sequential_focusable =
            trigger.sequential_focusable.unwrap_or(selected) && !trigger.disabled;
        let surface: Widget = TabTriggerSurface {
            id: trigger.id,
            panel_id: trigger.panel_id,
            label,
            semantics_identifier: trigger.semantics_identifier,
            action: trigger.on_press,
            selected,
            disabled: trigger.disabled,
            sequential_focusable,
            style,
            child,
        }
        .into();

        let indicator_height = theme.indicator_height_for(trigger.presentation);
        if indicator_height <= 0.0 {
            return surface;
        }
        let mut indicator: Widget = if selected {
            Container::new(fission_core::ui::widgets::spacer::Spacer::default())
                .height(indicator_height)
                .bg_fill(indicator_fill)
                .into()
        } else {
            fission_core::ui::widgets::spacer::Spacer {
                height: Some(indicator_height),
                ..Default::default()
            }
            .into()
        };
        if selected && !trigger.indicator_tracks.is_empty() {
            indicator = Motion {
                id: trigger
                    .indicator_slot
                    .unwrap_or_else(|| slot_id(trigger.id, SLOT_INDICATOR)),
                tracks: trigger.indicator_tracks,
                child: indicator,
                ..Default::default()
            }
            .into();
        }
        VStack {
            spacing: Some(0.0),
            children: vec![surface, indicator],
        }
        .into()
    }
}

/// Retained horizontal tab-list anatomy.
///
/// The list owns the `tablist` role, logical ordering, and the active track
/// recipe. Its children remain [`TabTrigger`] values so arbitrary buttons
/// cannot silently weaken the composite's semantics. When at least one trigger
/// is enabled, the first enabled selected trigger (or the first enabled trigger
/// when none is selected) becomes the only rendered selection and the only
/// participant in page-level `Tab` traversal. This normalization does not
/// dispatch actions or mutate application state. A list containing only
/// disabled triggers has no rendered selection or tab stop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabList {
    /// Stable identity for the list relationship and retained surface.
    pub id: WidgetId,
    /// Tab triggers in logical navigation order.
    pub tabs: Vec<TabTrigger>,
}

impl TabList {
    /// Creates a horizontal list of semantic tab triggers.
    pub fn new(id: WidgetId, tabs: Vec<TabTrigger>) -> Self {
        Self { id, tabs }
    }

    /// Applies one named design-system treatment to every trigger and the list track.
    pub fn presentation(mut self, presentation: TabPresentation) -> Self {
        for tab in &mut self.tabs {
            tab.presentation = presentation;
        }
        self
    }
}

impl From<TabList> for Widget {
    fn from(mut list: TabList) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        normalize_selection_and_sequential_focus(&mut list.tabs);
        let presentation = list
            .tabs
            .first()
            .map(|tab| tab.presentation)
            .unwrap_or_default();
        let surface: Widget = TabListSurface {
            id: list.id,
            style: view
                .env()
                .theme
                .components
                .tabs
                .track_style_for(presentation)
                .clone(),
            fallback_background: Fill::Solid(view.env().theme.components.tabs.background),
            children: list.tabs.into_iter().map(Widget::from).collect(),
        }
        .into();

        Scroll {
            id: Some(WidgetId::derived(list.id.as_u128(), &[0x5343_524c])),
            child: Some(surface),
            direction: FlexDirection::Row,
            show_scrollbar: false,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()
    }
}

/// Retained active-panel anatomy for a tab trigger.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabPanel {
    /// Stable panel identity referenced by its controlling trigger.
    pub id: WidgetId,
    /// Stable identity of the trigger that labels this panel.
    pub labelled_by: WidgetId,
    /// Retained panel content.
    pub child: Widget,
}

impl TabPanel {
    /// Creates an active panel and its accessible trigger relationship.
    pub fn new(id: WidgetId, labelled_by: WidgetId, child: impl Into<Widget>) -> Self {
        Self {
            id,
            labelled_by,
            child: child.into(),
        }
    }
}

impl From<TabPanel> for Widget {
    fn from(panel: TabPanel) -> Self {
        fission_core::ui::SemanticsRegion {
            id: Some(panel.id),
            role: Role::TabPanel,
            labelled_by: vec![panel.labelled_by],
            child: Some(panel.child),
            ..Default::default()
        }
        .into()
    }
}

/// Complete retained tabs anatomy joining a list and active panel content.
///
/// `content` commonly contains [`TabPanel`] and may be wrapped in [`Presence`]
/// or another retained transition without rebuilding list semantics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabsLayout {
    /// Retained tab-list anatomy.
    pub list: TabList,
    /// Active panel or retained panel transition.
    pub content: Widget,
}

impl TabsLayout {
    /// Creates complete tabs anatomy from its list and active panel content.
    pub fn new(list: TabList, content: impl Into<Widget>) -> Self {
        Self {
            list,
            content: content.into(),
        }
    }
}

impl From<TabsLayout> for Widget {
    fn from(layout: TabsLayout) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        VStack {
            spacing: Some(view.env().theme.tokens.spacing.s),
            children: vec![layout.list.into(), layout.content],
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct TabListSurface {
    id: WidgetId,
    style: ResolvedComponentStyle,
    fallback_background: Fill,
    children: Vec<Widget>,
}

impl From<TabListSurface> for Widget {
    fn from(surface: TabListSurface) -> Self {
        fission_core::authoring::custom_widget("Tabs.TabList", surface)
    }
}

impl LowerWidget for TabListSurface {
    fn lower_dyn(&self, cx: &mut LoweringCx) -> WidgetId {
        cx.push_scope(self.id);
        let layout_id = cx.next_node_id();
        cx.push_scope(layout_id);

        let mut row = IrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
                wrap: FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: Some(self.style.gap.unwrap_or(14.0)),
                line_gap: None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
            }),
        );
        for child in &self.children {
            row.add_child(fission_core::internal::lower_widget(child, cx));
        }
        let row_id = row.build(cx);

        let mut layout = IrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::StyledBox {
                style: recipe_box_style(&self.style, [2.0; 4], BoxAlignment::Start, None),
                flex_grow: 0.0,
                flex_shrink: 1.0,
            }),
        )
        .composite(recipe_composite_style(&self.style));
        append_recipe_paint(
            &mut layout,
            cx,
            &self.style,
            Some(self.fallback_background.clone()),
        );
        layout.add_child(row_id);
        let layout_id = layout.build(cx);

        cx.pop_scope();
        cx.pop_scope();

        let mut semantics = IrBuilder::new(
            self.id,
            Op::Semantics(Semantics {
                role: Role::TabList,
                orientation: Some(SemanticOrientation::Horizontal),
                ..Default::default()
            }),
        );
        semantics.add_child(layout_id);
        semantics.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(WidgetId::derived(
            self.id.as_u128(),
            &[CUSTOM_WRAPPER_ID_SALT],
        ))
    }

    fn stable_key(&self) -> u64 {
        stable_id_key(self.id)
    }
}

#[derive(Clone, Debug)]
struct TabTriggerSurface {
    id: WidgetId,
    panel_id: WidgetId,
    label: String,
    semantics_identifier: Option<String>,
    action: Option<ActionEnvelope>,
    selected: bool,
    disabled: bool,
    sequential_focusable: bool,
    style: ResolvedComponentStyle,
    child: Widget,
}

impl From<TabTriggerSurface> for Widget {
    fn from(surface: TabTriggerSurface) -> Self {
        fission_core::authoring::custom_widget("Tabs.Tab", surface)
    }
}

impl LowerWidget for TabTriggerSurface {
    fn lower_dyn(&self, cx: &mut LoweringCx) -> WidgetId {
        cx.push_scope(self.id);
        let layout_id = cx.next_node_id();
        cx.push_scope(layout_id);

        let mut content = IrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Row,
                wrap: FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: Some(self.style.gap.unwrap_or(0.0)),
                line_gap: None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }),
        );
        content.add_child(fission_core::internal::lower_widget(&self.child, cx));
        let content_id = content.build(cx);

        let mut layout = IrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::StyledBox {
                style: recipe_box_style(
                    &self.style,
                    [10.0, 10.0, 0.0, 0.0],
                    BoxAlignment::Center,
                    Some(38.0),
                ),
                flex_grow: 0.0,
                flex_shrink: 1.0,
            }),
        )
        .composite(recipe_composite_style(&self.style));
        append_recipe_paint(&mut layout, cx, &self.style, None);
        layout.add_child(content_id);
        let layout_id = layout.build(cx);

        cx.pop_scope();
        cx.pop_scope();

        let mut actions = ActionSet::default();
        if !self.disabled {
            if let Some(action) = &self.action {
                actions.entries.push(ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: action.id.as_u128(),
                    payload_data: Some(action.payload.clone()),
                });
            }
        }
        let mut semantics = IrBuilder::new(
            self.id,
            Op::Semantics(Semantics {
                role: Role::Tab,
                identifier: self.semantics_identifier.clone(),
                label: Some(self.label.clone()),
                actions,
                focusable: !self.disabled,
                disabled: self.disabled,
                selected: Some(self.selected),
                controls: vec![self.panel_id],
                sequential_focusable: self.sequential_focusable,
                ..Default::default()
            }),
        );
        semantics.add_child(layout_id);
        semantics.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(WidgetId::derived(
            self.id.as_u128(),
            &[CUSTOM_WRAPPER_ID_SALT],
        ))
    }

    fn stable_key(&self) -> u64 {
        stable_id_key(self.id)
    }
}

fn normalize_selection_and_sequential_focus(tabs: &mut [TabTrigger]) {
    let target = tabs
        .iter()
        .position(|tab| tab.selected && !tab.disabled)
        .or_else(|| tabs.iter().position(|tab| !tab.disabled));

    for (index, tab) in tabs.iter_mut().enumerate() {
        let selected = target == Some(index);
        tab.selected = selected;
        tab.sequential_focusable = Some(selected);
    }
}

fn recipe_box_style(
    style: &ResolvedComponentStyle,
    fallback_padding: [f32; 4],
    alignment: BoxAlignment,
    fallback_height: Option<f32>,
) -> BoxStyle {
    BoxStyle {
        width: style.width.map(Length::Points),
        height: style.height.or(fallback_height).map(Length::Points),
        min_height: style.min_height.map(Length::Points),
        max_width: style.max_width.map(Length::Points),
        padding: Some(
            style
                .padding_box(fallback_padding[0], fallback_padding[2])
                .map(Length::Points),
        ),
        alignment,
        ..Default::default()
    }
}

fn append_recipe_paint(
    layout: &mut IrBuilder,
    cx: &mut LoweringCx,
    style: &ResolvedComponentStyle,
    fallback_background: Option<Fill>,
) {
    let radius = style.radius.unwrap_or(0.0);
    for shadow in &style.shadows {
        layout.add_child(
            IrBuilder::new(
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

    let fill = style.background.clone().or(fallback_background);
    let stroke = style.border.as_ref().map(|border| Stroke {
        fill: border.fill.clone(),
        width: border.width,
        dash_array: style.border_dash.clone(),
        line_cap: fission_core::op::LineCap::Butt,
        line_join: fission_core::op::LineJoin::Miter,
    });
    if fill.is_some() || stroke.is_some() {
        layout.add_child(
            IrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill,
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

impl From<Tabs> for Widget {
    fn from(component: Tabs) -> Self {
        let this = &component;
        let base_id = fission_core::build::current_widget_id()
            .or_else(|| fission_core::build::next_implicit_widget_id(IMPLICIT_TABS_ID_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.tabs.motion"));
        let resolved_active_index = if this.items.is_empty() {
            None
        } else if this.active_index < this.items.len() {
            Some(this.active_index)
        } else {
            Some(0)
        };
        let active_index = resolved_active_index.unwrap_or(0);
        let active_trigger = WidgetId::derived(base_id.as_u128(), &[active_index as u32, 0]);
        let motion_plan = this
            .motion
            .as_ref()
            .map(|motion| motion.plan(active_trigger));
        let indicator_slot = slot_id(base_id, SLOT_INDICATOR);
        let tab_triggers = this
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let trigger_id = WidgetId::derived(base_id.as_u128(), &[index as u32, 0]);
                let panel_id =
                    WidgetId::derived(base_id.as_u128(), &[index as u32, TAB_PANEL_ID_SALT]);
                let mut trigger = TabTrigger::new(trigger_id, panel_id, item.title.clone())
                    .selected(Some(index) == resolved_active_index)
                    .size(this.size);
                trigger.on_press = item.on_press.clone();
                trigger.semantics_identifier = item.semantics_identifier.clone();
                if Some(index) == resolved_active_index {
                    if let Some(plan) = &motion_plan {
                        trigger = trigger.indicator_tracks(indicator_slot, plan.indicator.clone());
                    }
                }
                trigger
            })
            .collect();
        let tab_list = TabList::new(
            WidgetId::derived(base_id.as_u128(), &[TAB_LIST_ID_SALT]),
            tab_triggers,
        );

        let mut active_content = if let Some(active_index) = resolved_active_index {
            let tab = &this.items[active_index];
            TabPanel::new(
                WidgetId::derived(base_id.as_u128(), &[active_index as u32, TAB_PANEL_ID_SALT]),
                active_trigger,
                tab.content.clone(),
            )
            .into()
        } else {
            fission_core::ui::widgets::spacer::Spacer::default().into()
        };
        active_content = Composite {
            id: Some(WidgetId::derived(
                slot_id(base_id, SLOT_CONTENT).as_u128(),
                &[active_index as u32, 0],
            )),
            child: active_content,
            ..Default::default()
        }
        .into();
        if let Some(plan) = &motion_plan {
            active_content = Presence {
                id: WidgetId::derived(
                    slot_id(base_id, SLOT_CONTENT).as_u128(),
                    &[active_index as u32],
                ),
                visible: true,
                enter: plan.content_enter.clone(),
                exit: plan.content_exit.clone(),
                keep_rendered: plan.keep_rendered,
                child: active_content,
                ..Default::default()
            }
            .into();
        }

        TabsLayout::new(tab_list, active_content).into()
    }
}
