use crate::authoring::Lower;
use crate::lowering::{IrBuilder, LoweringCx};
use crate::motion::{
    color, fill as motion_fill, hover_press, px, ripple_effect, scalar, shadows as motion_shadows,
    MotionEasing, MotionExpr, MotionPhase, MotionPredicate, MotionPropertyId, MotionStartValue,
    MotionTrack, MotionTransition, MotionValue, RippleFx,
};
use crate::ui::{Icon, Text, TextContent, Widget};
use crate::{ActionEnvelope, Env, InteractionStateMap};
use fission_ir::{
    op::{BoxShadow, Color as IrColor, Fill, LayoutOp, Op, PaintOp, Stroke},
    ActionEntry, ActionSet, CompositeScalar, CompositeStyle, FocusPolicy, Role, Semantics,
    TextFieldValidationState, WidgetId,
};
use fission_theme::{
    ButtonHierarchy, ComponentMotion, ComponentSize, ComponentState, ComponentStateStyles,
    EasingCurve, ResolvedComponentStyle,
};
use serde::{Deserialize, Serialize};
use std::ops::Add;

/// Visual style variant for a [`Button`].
///
/// - `Filled` -- solid background with the primary colour (default).
/// - `Outline` -- transparent background with a border stroke.
/// - `Ghost` -- no background or border; just text/icon.
///
/// # Example
///
/// ```rust,ignore
/// Button {
///     variant: ButtonVariant::Outline,
///     child: Some(Text::new("Cancel").into()),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ButtonVariant {
    /// Solid primary-colour background.
    #[default]
    Filled,
    /// Transparent background with a border.
    Outline,
    /// No background, no border.
    Ghost,
    /// DSP primary hierarchy.
    Primary,
    /// DSP secondary colour hierarchy.
    SecondaryColor,
    /// DSP secondary gray hierarchy.
    SecondaryGray,
    /// DSP tertiary colour hierarchy.
    TertiaryColor,
    /// DSP tertiary gray hierarchy.
    TertiaryGray,
    /// DSP link colour hierarchy.
    LinkColor,
    /// DSP link gray hierarchy.
    LinkGray,
    /// DSP destructive hierarchy.
    Destructive,
}

impl ButtonVariant {
    fn hierarchy(self) -> ButtonHierarchy {
        match self {
            ButtonVariant::Filled | ButtonVariant::Primary => ButtonHierarchy::Primary,
            ButtonVariant::Outline => ButtonHierarchy::Outline,
            ButtonVariant::SecondaryGray => ButtonHierarchy::SecondaryGray,
            ButtonVariant::Ghost | ButtonVariant::TertiaryGray => ButtonHierarchy::TertiaryGray,
            ButtonVariant::SecondaryColor => ButtonHierarchy::SecondaryColor,
            ButtonVariant::TertiaryColor => ButtonHierarchy::TertiaryColor,
            ButtonVariant::LinkColor => ButtonHierarchy::LinkColor,
            ButtonVariant::LinkGray => ButtonHierarchy::LinkGray,
            ButtonVariant::Destructive => ButtonHierarchy::Destructive,
        }
    }
}

/// Horizontal alignment of a [`Button`]'s child content.
///
/// Defaults to `Center`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ButtonContentAlign {
    /// Center the child horizontally and vertically (default).
    #[default]
    Center,
    /// Align the child to the leading edge.
    Start,
    /// Align the child to the trailing edge.
    End,
}

/// Retained, recipe-aware content for a [`Button`].
///
/// Use this anatomy for the common icon-and-label composition. The button
/// applies its resolved typography, foreground colour, icon size, and gap to
/// these slots while preserving explicit icon overrides. Arbitrary custom
/// content remains available through [`Button::child`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonContent {
    /// Visible and accessible button label.
    pub label: TextContent,
    /// Optional icon before the label in logical reading order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leading_icon: Option<Icon>,
    /// Optional icon after the label in logical reading order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing_icon: Option<Icon>,
}

impl ButtonContent {
    /// Creates labelled button content.
    pub fn new(label: impl Into<TextContent>) -> Self {
        Self {
            label: label.into(),
            leading_icon: None,
            trailing_icon: None,
        }
    }

    /// Adds an icon before the label in logical reading order.
    pub fn leading_icon(mut self, icon: Icon) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// Adds an icon after the label in logical reading order.
    pub fn trailing_icon(mut self, icon: Icon) -> Self {
        self.trailing_icon = Some(icon);
        self
    }
}

/// Retained, recipe-aware icon-only content for a [`Button`].
///
/// The icon inherits the button recipe's foreground colour and icon size while
/// `accessible_label` names the control without adding visible text. Explicit
/// colour and size values on the icon continue to take precedence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonIconContent {
    /// Visible icon rendered by the button.
    pub icon: Icon,
    /// Accessible name for the icon-only control.
    pub accessible_label: TextContent,
}

impl ButtonIconContent {
    /// Creates recipe-aware icon-only button content with an accessible name.
    pub fn new(icon: Icon, accessible_label: impl Into<TextContent>) -> Self {
        Self {
            icon,
            accessible_label: accessible_label.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Button`].
///
/// The active button recipe automatically owns transitions between its visual
/// states. Set [`Button::motion`] to `Some(...)` to compose optional transform
/// or ripple feedback with those recipe transitions.
///
/// ```rust,ignore
/// use fission::prelude::*;
///
/// Button {
///     id: Some(WidgetId::explicit("save")),
///     child: Some(Text::new("Save").into()),
///     motion: Some(ButtonMotion::HoverScale + ButtonMotion::PressScale + ButtonMotion::Ripple),
///     ..Default::default()
/// };
/// ```
pub enum ButtonMotion {
    /// Curated default hover/press scale feedback.
    Default,
    /// Scale up slightly while hovered.
    HoverScale,
    /// Scale down slightly while pressed.
    PressScale,
    /// Compound hover plus press scale feedback.
    HoverPressScale,
    /// Add deterministic pointer-origin ripples.
    Ripple,
    /// Compound hover/press scale plus ripple feedback.
    HoverPressRipple,
    /// Ordered composition of button motion atoms.
    Composition(Vec<ButtonMotion>),
    /// Caller-provided native interaction tracks and ripple configuration.
    Custom {
        /// Interaction tracks for the button root slot.
        interaction: Option<Vec<MotionTrack>>,
        /// Optional ripple effect for the ripple slot.
        ripple: Option<RippleFx>,
    },
}

impl ButtonMotion {
    /// Flattens and normalizes an ordered button-motion composition.
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

    /// Lowers this preset into interaction tracks for `id`.
    pub fn interaction_tracks(&self, id: WidgetId) -> Vec<MotionTrack> {
        let mut tracks = Vec::new();
        self.append_interaction_tracks(id, &mut tracks);
        crate::motion::dedupe_tracks_later_wins(tracks)
    }

    fn append_interaction_tracks(&self, id: WidgetId, out: &mut Vec<MotionTrack>) {
        match self {
            Self::Default | Self::HoverPressScale => out.extend(hover_press(id)),
            Self::HoverScale => out.push(
                MotionTrack::composite(
                    MotionPropertyId::Scale,
                    MotionStartValue::Current,
                    MotionExpr::If {
                        predicate: MotionPredicate::Hovered(id),
                        then_expr: Box::new(scalar(1.02)),
                        else_expr: Box::new(scalar(1.0)),
                    },
                )
                .transition(MotionTransition::spring(420.0, 30.0)),
            ),
            Self::PressScale => out.push(
                MotionTrack::composite(
                    MotionPropertyId::Scale,
                    MotionStartValue::Current,
                    MotionExpr::If {
                        predicate: MotionPredicate::Pressed(id),
                        then_expr: Box::new(scalar(0.97)),
                        else_expr: Box::new(scalar(1.0)),
                    },
                )
                .transition(MotionTransition::spring(420.0, 30.0)),
            ),
            Self::Ripple => {}
            Self::HoverPressRipple => out.extend(hover_press(id)),
            Self::Composition(items) => {
                for item in items {
                    item.append_interaction_tracks(id, out);
                }
            }
            Self::Custom { interaction, .. } => out.extend(interaction.clone().unwrap_or_default()),
        }
    }

    /// Returns the ripple effect selected by this preset, if any.
    pub fn ripple(&self) -> Option<RippleFx> {
        match self {
            Self::Ripple | Self::HoverPressRipple => Some(ripple_effect()),
            Self::Composition(items) => items.iter().rev().find_map(Self::ripple),
            Self::Custom { ripple, .. } => ripple.clone(),
            _ => None,
        }
    }
}

impl Add for ButtonMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

/// A pressable button widget with built-in theming, hover/press states, and
/// focus ring.
///
/// Buttons expose compatibility variants and the complete design-system
/// hierarchy through [`ButtonVariant`]. Use [`ButtonContent`] for the common
/// labelled/icon anatomy, [`ButtonIconContent`] for an icon-only control with
/// an independent accessible name, or [`Button::child`] for genuinely custom
/// content.
///
/// # Example
///
/// ```rust,ignore
/// let on_press = ctx.bind(Submit, reduce_with!(handle_submit));
///
/// Button {
///     child: Some(Text::new("Submit").into()),
///     on_press: Some(on_press),
///     variant: ButtonVariant::Filled,
///     content_align: ButtonContentAlign::Center,
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Button {
    /// Explicit node identity (auto-generated if `None`).
    pub id: Option<WidgetId>,
    /// The button's content widget (typically [`crate::ui::Text`] or
    /// [`crate::ui::Icon`]).
    pub child: Option<Widget>,
    /// Recipe-aware label and icon anatomy.
    ///
    /// When present, this takes precedence over [`Button::child`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<ButtonContent>,
    /// Recipe-aware icon-only anatomy and its independent accessible name.
    ///
    /// When present, this takes precedence over [`Button::content`] and
    /// [`Button::child`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_content: Option<ButtonIconContent>,
    /// Action dispatched when the button is pressed.
    pub on_press: Option<ActionEnvelope>,
    /// Custom semantics (overrides the default button semantics).
    pub semantics: Option<Semantics>,
    /// How pointer-down should affect focus for this button.
    ///
    /// Use [`FocusPolicy::PreserveCurrentOnPointer`] for toolbar/ribbon buttons
    /// that should activate without stealing focus from an editor.
    #[serde(default)]
    pub focus_policy: FocusPolicy,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Minimum width constraint.
    pub min_width: Option<f32>,
    /// Maximum width constraint.
    pub max_width: Option<f32>,
    /// Flex grow factor for parent flex layouts.
    pub flex_grow: f32,
    /// Flex shrink factor for parent flex layouts.
    pub flex_shrink: f32,
    /// Custom padding `[left, right, top, bottom]` (overrides theme defaults).
    pub padding: Option<[f32; 4]>,
    /// Optional visual overrides applied after the active design-system recipe.
    pub style: Option<ButtonStyleOverride>,
    /// Visual variant or design-system hierarchy.
    pub variant: ButtonVariant,
    /// Design-system size slot.
    #[serde(default)]
    pub size: ComponentSize,
    /// Optional fill override for the button background.
    pub background_fill: Option<Fill>,
    /// Optional text color override for direct `Text` children.
    pub text_color: Option<IrColor>,
    /// Horizontal alignment of the child content.
    #[serde(default)]
    pub content_align: ButtonContentAlign,
    /// When `true`, the button is greyed out and its `on_press` action is not
    /// attached.
    pub disabled: bool,
    /// Optional transform and ripple motion composed with recipe state transitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<ButtonMotion>,
}

impl Button {
    /// Sets the stable identifier exposed on the button's semantics node.
    ///
    /// This preserves any custom semantics already configured on the button.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        let semantics = self.semantics.get_or_insert_with(default_button_semantics);
        semantics.identifier = Some(identifier.into());
        self
    }

    /// Associates this submit action with named controls in one logical form.
    ///
    /// Server-rendered HTML uses this id to bind the signed action to its typed
    /// form schema. Text inputs submitted by the action use the same `form_id`.
    pub fn form_id(mut self, form_id: impl Into<String>) -> Self {
        let semantics = self.semantics.get_or_insert_with(default_button_semantics);
        semantics.text_form_id = Some(form_id.into());
        self
    }

    pub fn background_fill(mut self, fill: Fill) -> Self {
        self.background_fill = Some(fill);
        self
    }

    pub fn text_color(mut self, color: IrColor) -> Self {
        self.text_color = Some(color);
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow;
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = shrink;
        self
    }

    /// Sets how pointer-down should affect focus for this button.
    pub fn focus_policy(mut self, focus_policy: FocusPolicy) -> Self {
        self.focus_policy = focus_policy;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width);
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }
}

impl Default for Button {
    fn default() -> Self {
        Self {
            id: None,
            child: None,
            content: None,
            icon_content: None,
            on_press: None,
            semantics: None,
            focus_policy: FocusPolicy::FocusOnPointer,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: None,
            style: None,
            variant: ButtonVariant::Filled,
            size: ComponentSize::Md,
            background_fill: None,
            text_color: None,
            content_align: ButtonContentAlign::Center,
            disabled: false,
            motion: None,
        }
    }
}

/// Targeted visual overrides for one [`Button`].
///
/// The active design-system recipe remains the base authority. These values
/// are useful for a genuinely exceptional control without requiring callers
/// to rebuild button interaction, focus, and semantics from lower-level
/// widgets. Dedicated fields on [`Button`] take precedence where both APIs
/// provide the same value.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonStyleOverride {
    /// Background fill replacing the recipe fill.
    pub background_fill: Option<Fill>,
    /// Foreground applied to a direct text child.
    pub text_color: Option<IrColor>,
    /// Border stroke replacing the recipe border.
    pub border: Option<Stroke>,
    /// Preferred width used when [`Button::width`] is absent.
    pub width: Option<f32>,
    /// Maximum width used when [`Button::max_width`] is absent.
    pub max_width: Option<f32>,
    /// Control height used when [`Button::height`] is absent.
    pub height: Option<f32>,
    /// Content padding used when [`Button::padding`] is absent.
    pub padding: Option<[f32; 4]>,
    /// Surface and shadow corner radius.
    pub corner_radius: Option<f32>,
    /// Replaces the recipe's complete shadow stack when present. An empty
    /// vector explicitly requests a flat surface.
    pub shadows: Option<Vec<BoxShadow>>,
    /// Direct text-child font size.
    pub font_size: Option<f32>,
    /// Direct text-child font weight.
    pub font_weight: Option<u16>,
    /// Direct text-child line height.
    pub line_height: Option<f32>,
    /// Direct text-child letter spacing.
    pub letter_spacing: Option<f32>,
    /// Minimum control height used when [`Button::height`] is absent.
    pub min_height: Option<f32>,
    /// Gap between recipe-aware label and icon anatomy.
    pub gap: Option<f32>,
    /// Default size for recipe-aware icon anatomy.
    pub icon_size: Option<f32>,
    /// Opacity applied to the complete interactive surface.
    pub opacity: Option<f32>,
    /// Vertical translation applied to the complete interactive surface.
    pub translate_y: Option<f32>,
    /// Component transition used for state changes.
    pub transition: Option<ComponentMotion>,
    /// Component-specific base recipe used before state overlays.
    ///
    /// Composite controls use this with [`Self::state_styles`] so their own
    /// design-system recipe remains authoritative while retaining Button's
    /// focus, pointer, keyboard, and semantic behavior.
    pub recipe_base: Option<ResolvedComponentStyle>,
    /// Component-specific interaction state overlays.
    pub state_styles: Option<ComponentStateStyles>,
    /// Prevents values from the ordinary Button hierarchy leaking into a
    /// component-specific recipe.
    pub replace_component_recipe: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct ButtonStyleResolved {
    background_fill: Option<Fill>,
    text_color: IrColor,
    width: Option<f32>,
    max_width: Option<f32>,
    padding: [f32; 4],
    height: f32,
    min_height: Option<f32>,
    corner_radius: f32,
    outer_shadows: Vec<BoxShadow>,
    inset_shadows: Vec<BoxShadow>,
    stroke: Option<Stroke>,
    focus_ring: Option<Stroke>,
    layout_border_width: f32,
    font_size: f32,
    font_weight: u16,
    line_height: Option<f32>,
    letter_spacing: Option<f32>,
    gap: f32,
    icon_size: f32,
    opacity: f32,
    translate_y: f32,
}

impl ButtonContent {
    fn lower_with_style(
        &self,
        cx: &mut LoweringCx<'_>,
        style: &ButtonStyleResolved,
        button_id: WidgetId,
    ) -> WidgetId {
        let mut row = IrBuilder::new(
            WidgetId::derived(button_id.as_u128(), &[0xBC00]),
            Op::Layout(LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Row,
                wrap: fission_ir::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: Some(style.gap),
                line_gap: None,
                align_items: fission_ir::op::AlignItems::Center,
                justify_content: fission_ir::op::JustifyContent::Center,
            }),
        );

        if let Some(icon) = &self.leading_icon {
            row.add_child(lower_content_icon(icon, cx, style, button_id, 0));
        }

        let mut label = Text::new(self.label.clone())
            .size(style.font_size)
            .weight(style.font_weight)
            .color(style.text_color);
        if let Some(line_height) = style.line_height {
            label = label.line_height(line_height);
        }
        if let Some(letter_spacing) = style.letter_spacing {
            label = label.letter_spacing(letter_spacing);
        }
        label.id = Some(WidgetId::derived(button_id.as_u128(), &[0xBC01]));
        row.add_child(label.lower(cx));

        if let Some(icon) = &self.trailing_icon {
            row.add_child(lower_content_icon(icon, cx, style, button_id, 1));
        }

        row.build(cx)
    }
}

impl ButtonIconContent {
    fn lower_with_style(
        &self,
        cx: &mut LoweringCx<'_>,
        style: &ButtonStyleResolved,
        button_id: WidgetId,
    ) -> WidgetId {
        lower_content_icon(&self.icon, cx, style, button_id, 0)
    }
}

fn lower_content_icon(
    icon: &Icon,
    cx: &mut LoweringCx<'_>,
    style: &ButtonStyleResolved,
    button_id: WidgetId,
    slot: u32,
) -> WidgetId {
    let mut icon = icon.clone();
    if icon.id.is_none() {
        icon.id = Some(WidgetId::derived(button_id.as_u128(), &[0xBC02, slot]));
    }
    if icon.color.is_none() {
        icon.color = Some(style.text_color);
    }
    if icon.size.is_none() {
        icon.size = Some(style.icon_size);
    }
    icon.lower(cx)
}

#[derive(Clone)]
struct ButtonStateValues<T> {
    rest: T,
    hover: T,
    pressed: T,
    focused: T,
    hover_focused: T,
    pressed_focused: T,
    disabled: T,
}

impl<T: PartialEq> ButtonStateValues<T> {
    fn varies(&self) -> bool {
        self.hover != self.rest
            || self.pressed != self.rest
            || self.focused != self.rest
            || self.hover_focused != self.rest
            || self.pressed_focused != self.rest
            || self.disabled != self.rest
    }
}

impl<T> ButtonStateValues<T> {
    fn map<U>(&self, map: impl Fn(&T) -> U) -> ButtonStateValues<U> {
        ButtonStateValues {
            rest: map(&self.rest),
            hover: map(&self.hover),
            pressed: map(&self.pressed),
            focused: map(&self.focused),
            hover_focused: map(&self.hover_focused),
            pressed_focused: map(&self.pressed_focused),
            disabled: map(&self.disabled),
        }
    }

    fn into_motion_expr(
        self,
        id: WidgetId,
        disabled: bool,
        expression: impl Fn(T) -> MotionExpr,
    ) -> MotionExpr {
        if disabled {
            return expression(self.disabled);
        }

        let unfocused = MotionExpr::If {
            predicate: MotionPredicate::Pressed(id),
            then_expr: Box::new(expression(self.pressed)),
            else_expr: Box::new(MotionExpr::If {
                predicate: MotionPredicate::Hovered(id),
                then_expr: Box::new(expression(self.hover)),
                else_expr: Box::new(expression(self.rest)),
            }),
        };
        let focused = MotionExpr::If {
            predicate: MotionPredicate::Pressed(id),
            then_expr: Box::new(expression(self.pressed_focused)),
            else_expr: Box::new(MotionExpr::If {
                predicate: MotionPredicate::Hovered(id),
                then_expr: Box::new(expression(self.hover_focused)),
                else_expr: Box::new(expression(self.focused)),
            }),
        };
        MotionExpr::If {
            predicate: MotionPredicate::Focused(id),
            then_expr: Box::new(focused),
            else_expr: Box::new(unfocused),
        }
    }
}

impl<T> ButtonStateValues<Option<T>> {
    fn transpose(self) -> Option<ButtonStateValues<T>> {
        Some(ButtonStateValues {
            rest: self.rest?,
            hover: self.hover?,
            pressed: self.pressed?,
            focused: self.focused?,
            hover_focused: self.hover_focused?,
            pressed_focused: self.pressed_focused?,
            disabled: self.disabled?,
        })
    }
}

impl Button {
    pub(crate) const MOTION_SALT: u32 = 0xB0770;

    fn is_semantically_invalid(&self) -> bool {
        self.semantics.as_ref().is_some_and(|semantics| {
            semantics.validation_state == TextFieldValidationState::Invalid
        })
    }

    fn is_semantically_selected(&self) -> bool {
        self.semantics.as_ref().is_some_and(|semantics| {
            semantics.selected == Some(true) || semantics.expanded == Some(true)
        })
    }

    fn resolve_style(
        &self,
        env: &Env,
        interaction: &InteractionStateMap,
        self_id: WidgetId,
        form_field_invalid: bool,
    ) -> ButtonStyleResolved {
        let is_hovered = interaction.is_hovered(self_id) && !self.disabled;
        let is_pressed = interaction.is_pressed(self_id) && !self.disabled;
        let is_focused = interaction.is_focused(self_id) && !self.disabled;
        let is_invalid = !self.disabled && (form_field_invalid || self.is_semantically_invalid());
        let is_selected = !self.disabled && self.is_semantically_selected();
        // Pointer states and keyboard focus are independent. Resolve the
        // pointer/availability state first, then layer the focus recipe over
        // it so a focused button keeps its hover/pressed feedback and focus
        // ring at the same time.
        let component_state = if self.disabled {
            ComponentState::Disabled
        } else if is_invalid {
            ComponentState::Error
        } else if is_pressed {
            ComponentState::Active
        } else if is_hovered {
            ComponentState::Hover
        } else {
            ComponentState::Default
        };

        self.resolve_style_for_state(env, component_state, is_focused, is_selected)
    }

    fn resolve_style_for_state(
        &self,
        env: &Env,
        component_state: ComponentState,
        is_focused: bool,
        is_selected: bool,
    ) -> ButtonStyleResolved {
        let default_style = &env.theme.components.button;
        let tokens = &env.theme.tokens.colors;
        let is_disabled = component_state == ComponentState::Disabled;
        let is_pressed = component_state == ComponentState::Active;
        let is_hovered = component_state == ComponentState::Hover;
        let hierarchy = self.variant.hierarchy();
        let style_override = self.style.as_ref();
        let replaces_component_recipe =
            style_override.is_some_and(|style| style.replace_component_recipe);
        let (mut component_style, focus_style) = if replaces_component_recipe {
            let mut style = style_override
                .and_then(|overrides| overrides.recipe_base.clone())
                .unwrap_or_default();
            let states = style_override.and_then(|overrides| overrides.state_styles.as_ref());
            if let Some(states) = states {
                style = style.merge(&states.default);
                if is_selected && !is_disabled && component_state != ComponentState::Error {
                    style = style.merge(&states.resolve(ComponentState::Selected));
                }
                if component_state != ComponentState::Default {
                    style = style.merge(&states.resolve(component_state));
                }
            }
            let focus = (is_focused && !is_disabled)
                .then(|| states.and_then(|states| states.focus.clone()))
                .flatten();
            (style, focus)
        } else {
            let mut style = default_style.resolve(hierarchy, self.size, ComponentState::Default);
            if is_selected && !is_disabled && component_state != ComponentState::Error {
                style = style.merge(
                    &default_style
                        .hierarchy_style(hierarchy)
                        .resolve(ComponentState::Selected),
                );
            }
            if component_state != ComponentState::Default {
                style = style.merge(&default_style.resolve(hierarchy, self.size, component_state));
            }
            let focus = (is_focused && !is_disabled)
                .then(|| default_style.hierarchy_style(hierarchy).focus)
                .flatten();
            (style, focus)
        };
        let surface_border = component_style.border.clone();
        if let Some(focus_style) = &focus_style {
            component_style = component_style.merge_composing_shadows(focus_style);
        }

        let mut stroke = surface_border.map(|border| Stroke {
            fill: border.fill,
            width: border.width,
            dash_array: component_style.border_dash.clone(),
            line_cap: fission_ir::op::LineCap::Butt,
            line_join: fission_ir::op::LineJoin::Miter,
        });
        let mut focus_ring = if is_focused && !is_disabled {
            focus_style
                .as_ref()
                .and_then(|style| style.border.clone())
                .map(|border| Stroke {
                    fill: border.fill,
                    width: border.width,
                    dash_array: None,
                    line_cap: fission_ir::op::LineCap::Round,
                    line_join: fission_ir::op::LineJoin::Round,
                })
                .or_else(|| {
                    (!replaces_component_recipe)
                        .then(|| default_style.focus_stroke.clone())
                        .flatten()
                })
        } else {
            None
        };
        if !replaces_component_recipe && hierarchy == ButtonHierarchy::Destructive {
            if let Some(ring) = &mut focus_ring {
                ring.fill = Fill::Solid(tokens.error);
            }
        }
        let (mut outer_shadows, mut inset_shadows): (Vec<BoxShadow>, Vec<BoxShadow>) =
            component_style
                .shadows
                .iter()
                .map(fission_theme::ShadowLayer::to_box_shadow)
                .partition(|shadow| !shadow.inset);
        if !replaces_component_recipe
            && hierarchy == ButtonHierarchy::Destructive
            && is_focused
            && !is_disabled
        {
            for shadow in outer_shadows.iter_mut().chain(&mut inset_shadows) {
                shadow.color = tokens.error.with_alpha(shadow.color.a);
            }
        }
        if !replaces_component_recipe
            && outer_shadows.is_empty()
            && inset_shadows.is_empty()
            && !is_disabled
        {
            let legacy_shadow =
                if matches!(self.variant, ButtonVariant::Filled | ButtonVariant::Primary) {
                    if is_pressed {
                        default_style.elevation_pressed
                    } else if is_hovered {
                        default_style.elevation_hover
                    } else {
                        default_style.elevation_rest
                    }
                } else {
                    None
                };
            if let Some(shadow) = legacy_shadow {
                outer_shadows.push(shadow);
            }
        }

        if let Some(shadows) = style_override.and_then(|style| style.shadows.as_ref()) {
            let (overridden_outer, overridden_inset): (Vec<_>, Vec<_>) =
                shadows.iter().copied().partition(|shadow| !shadow.inset);
            outer_shadows = overridden_outer;
            inset_shadows = overridden_inset;
        }
        if let Some(overridden_stroke) = style_override.and_then(|style| style.border.clone()) {
            stroke = Some(overridden_stroke);
        }
        let mut layout_border_width = style_override
            .and_then(|style| style.border.as_ref())
            .map(|stroke| stroke.width)
            .unwrap_or(0.0);
        if replaces_component_recipe {
            let recipe_base = style_override.and_then(|style| style.recipe_base.as_ref());
            let state_styles = style_override.and_then(|style| style.state_styles.as_ref());
            let candidates = std::iter::once(recipe_base.and_then(|style| style.border.as_ref()))
                .chain(std::iter::once(
                    state_styles.and_then(|states| states.default.border.as_ref()),
                ))
                .chain(state_styles.into_iter().flat_map(|states| {
                    [
                        states.hover.as_ref(),
                        states.active.as_ref(),
                        states.focus.as_ref(),
                        states.disabled.as_ref(),
                        states.error.as_ref(),
                        states.selected.as_ref(),
                    ]
                    .into_iter()
                    .map(|style| style.and_then(|style| style.border.as_ref()))
                }));
            for border in candidates.flatten() {
                if border.width.is_finite() {
                    layout_border_width = layout_border_width.max(border.width);
                }
            }
        } else {
            for state in [
                ComponentState::Default,
                ComponentState::Hover,
                ComponentState::Active,
                ComponentState::Focus,
                ComponentState::Disabled,
                ComponentState::Error,
                ComponentState::Selected,
            ] {
                if let Some(border) = default_style.resolve(hierarchy, self.size, state).border {
                    if border.width.is_finite() {
                        layout_border_width = layout_border_width.max(border.width);
                    }
                }
            }
            if let Some(width) = default_style
                .focus_stroke
                .as_ref()
                .map(|stroke| stroke.width)
                .filter(|width| width.is_finite())
            {
                layout_border_width = layout_border_width.max(width);
            }
        }
        layout_border_width = layout_border_width.max(0.0);

        ButtonStyleResolved {
            background_fill: self
                .background_fill
                .clone()
                .or_else(|| style_override.and_then(|style| style.background_fill.clone()))
                .or_else(|| component_style.background.clone()),
            text_color: self
                .text_color
                .or_else(|| style_override.and_then(|style| style.text_color))
                .unwrap_or(component_style.text_color.unwrap_or(tokens.primary)),
            width: style_override
                .and_then(|style| style.width)
                .or(component_style.width),
            max_width: style_override
                .and_then(|style| style.max_width)
                .or(component_style.max_width),
            padding: style_override
                .and_then(|style| style.padding)
                .unwrap_or_else(|| {
                    component_style.padding_box(
                        default_style.padding_horizontal,
                        default_style.padding_vertical,
                    )
                }),
            height: style_override
                .and_then(|style| style.height)
                .or(component_style.height)
                .unwrap_or(default_style.height),
            min_height: style_override
                .and_then(|style| style.min_height)
                .or(component_style.min_height),
            corner_radius: style_override
                .and_then(|style| style.corner_radius)
                .or(component_style.radius)
                .unwrap_or(default_style.radius),
            outer_shadows,
            inset_shadows,
            stroke,
            focus_ring,
            layout_border_width,
            font_size: style_override
                .and_then(|style| style.font_size)
                .or(component_style.font_size)
                .unwrap_or(default_style.text_size),
            font_weight: style_override
                .and_then(|style| style.font_weight)
                .or(component_style.font_weight)
                .unwrap_or(default_style.font_weight),
            line_height: style_override
                .and_then(|style| style.line_height)
                .or(component_style.line_height),
            letter_spacing: style_override
                .and_then(|style| style.letter_spacing)
                .or(component_style.letter_spacing),
            gap: style_override
                .and_then(|style| style.gap)
                .or(component_style.gap)
                .unwrap_or(6.0),
            icon_size: style_override
                .and_then(|style| style.icon_size)
                .or(component_style.icon_size)
                .unwrap_or(default_style.icon_size),
            opacity: style_override
                .and_then(|style| style.opacity)
                .or(component_style.opacity)
                .filter(|opacity| opacity.is_finite())
                .unwrap_or(if is_disabled { 0.5 } else { 1.0 })
                .clamp(0.0, 1.0),
            translate_y: style_override
                .and_then(|style| style.translate_y)
                .or(component_style.translate_y)
                .filter(|offset| offset.is_finite())
                .unwrap_or(if is_pressed { 1.0 } else { 0.0 }),
        }
    }

    fn interaction_styles(&self, env: &Env) -> ButtonStateValues<ButtonStyleResolved> {
        let selected = self.is_semantically_selected();
        let requested_state = |state| {
            if state != ComponentState::Disabled && self.is_semantically_invalid() {
                ComponentState::Error
            } else {
                state
            }
        };
        ButtonStateValues {
            rest: self.resolve_style_for_state(
                env,
                requested_state(ComponentState::Default),
                false,
                selected,
            ),
            hover: self.resolve_style_for_state(
                env,
                requested_state(ComponentState::Hover),
                false,
                selected,
            ),
            pressed: self.resolve_style_for_state(
                env,
                requested_state(ComponentState::Active),
                false,
                selected,
            ),
            focused: self.resolve_style_for_state(
                env,
                requested_state(ComponentState::Default),
                true,
                selected,
            ),
            hover_focused: self.resolve_style_for_state(
                env,
                requested_state(ComponentState::Hover),
                true,
                selected,
            ),
            pressed_focused: self.resolve_style_for_state(
                env,
                requested_state(ComponentState::Active),
                true,
                selected,
            ),
            disabled: self.resolve_style_for_state(env, ComponentState::Disabled, false, false),
        }
    }

    pub(crate) fn recipe_component_motion(&self, env: &Env) -> Option<ComponentMotion> {
        if let Some(style) = &self.style {
            let component_motion = style
                .transition
                .clone()
                .or_else(|| {
                    style
                        .recipe_base
                        .as_ref()
                        .and_then(|recipe| recipe.transition.clone())
                })
                .or_else(|| {
                    style.state_styles.as_ref().and_then(|states| {
                        [
                            ComponentState::Default,
                            ComponentState::Hover,
                            ComponentState::Active,
                            ComponentState::Focus,
                            ComponentState::Disabled,
                            ComponentState::Error,
                            ComponentState::Selected,
                        ]
                        .into_iter()
                        .find_map(|state| states.resolve(state).transition)
                    })
                });
            if component_motion.is_some() || style.replace_component_recipe {
                return component_motion;
            }
        }
        let theme = &env.theme.components.button;
        let hierarchy = self.variant.hierarchy();
        [
            ComponentState::Default,
            ComponentState::Hover,
            ComponentState::Active,
            ComponentState::Focus,
            ComponentState::Disabled,
        ]
        .into_iter()
        .find_map(|state| theme.resolve(hierarchy, self.size, state).transition)
    }

    pub(crate) fn recipe_motion_tracks(&self, env: &Env, id: WidgetId) -> Vec<MotionTrack> {
        let Some(component_motion) = self.recipe_component_motion(env) else {
            return Vec::new();
        };
        let transition = component_motion_transition(component_motion);
        let styles = self.interaction_styles(env);
        let mut tracks = Vec::new();

        let backgrounds = styles.map(|style| {
            style
                .background_fill
                .clone()
                .unwrap_or(Fill::Solid(IrColor::TRANSPARENT))
        });
        if let Some(background_colors) = backgrounds
            .map(|fill| match fill {
                Fill::Solid(color) => Some(*color),
                _ => None,
            })
            .transpose()
        {
            if let Some(track) = button_paint_track(
                MotionPropertyId::BackgroundColor,
                background_colors,
                id,
                self.disabled,
                color,
                transition.clone(),
            ) {
                tracks.push(track);
            }
        } else if let Some(track) = button_paint_track(
            MotionPropertyId::BackgroundFill,
            backgrounds,
            id,
            self.disabled,
            motion_fill,
            MotionTransition::Instant,
        ) {
            tracks.push(track);
        }

        if let Some(border_colors) = styles
            .map(
                |style| match style.stroke.as_ref().map(|stroke| &stroke.fill) {
                    Some(Fill::Solid(color)) => Some(*color),
                    None => Some(IrColor::TRANSPARENT),
                    Some(_) => None,
                },
            )
            .transpose()
        {
            if let Some(track) = button_paint_track(
                MotionPropertyId::BorderColor,
                border_colors,
                id,
                self.disabled,
                color,
                transition.clone(),
            ) {
                tracks.push(track);
            }
        }
        if let Some(track) = button_paint_track(
            MotionPropertyId::BorderWidth,
            styles.map(|style| {
                style
                    .stroke
                    .as_ref()
                    .map(|stroke| stroke.width)
                    .unwrap_or(0.0)
            }),
            id,
            self.disabled,
            px,
            transition.clone(),
        ) {
            tracks.push(track);
        }
        if let Some(track) = button_paint_track(
            MotionPropertyId::CornerRadius,
            styles.map(|style| style.corner_radius),
            id,
            self.disabled,
            px,
            transition.clone(),
        ) {
            tracks.push(track);
        }
        if let Some(track) = button_paint_track(
            MotionPropertyId::BoxShadows,
            styles.map(|style| {
                style
                    .outer_shadows
                    .iter()
                    .chain(&style.inset_shadows)
                    .copied()
                    .collect::<Vec<_>>()
            }),
            id,
            self.disabled,
            motion_shadows,
            MotionTransition::Instant,
        ) {
            tracks.push(track);
        }
        if let Some(track) = button_paint_track(
            MotionPropertyId::TextColor,
            styles.map(|style| style.text_color),
            id,
            self.disabled,
            color,
            transition.clone(),
        ) {
            tracks.push(track);
        }
        if let Some(track) = button_composite_track(
            MotionPropertyId::Opacity,
            styles.map(|style| style.opacity),
            id,
            self.disabled,
            scalar,
            transition.clone(),
        ) {
            tracks.push(track);
        }
        if let Some(track) = button_composite_track(
            MotionPropertyId::TranslateY,
            styles.map(|style| style.translate_y),
            id,
            self.disabled,
            scalar,
            transition,
        ) {
            tracks.push(track);
        }

        tracks
    }

    pub(crate) fn register_motion_declarations(&self, id: WidgetId) {
        let explicit_motion = self.motion.as_ref();
        let mut tracks = crate::build::try_current_env()
            .map(|env| self.recipe_motion_tracks(env, id))
            .unwrap_or_default();
        let ripple = explicit_motion.and_then(ButtonMotion::ripple);
        if let Some(motion) = explicit_motion {
            tracks.extend(motion.interaction_tracks(id));
        }
        let tracks = crate::motion::dedupe_tracks_later_wins(tracks);
        if !tracks.is_empty() {
            crate::build::try_register_motion(crate::motion::MotionDeclaration {
                id,
                kind: crate::motion::MotionDeclarationKind::Tracks { tracks },
            });
        }
        if let Some(effect) = ripple {
            crate::build::try_register_motion(crate::motion::MotionDeclaration {
                id,
                kind: crate::motion::MotionDeclarationKind::RippleLayer(effect),
            });
        }
    }

    fn animated_style(
        &self,
        cx: &LoweringCx<'_>,
        mut style: ButtonStyleResolved,
    ) -> ButtonStyleResolved {
        if self.recipe_component_motion(cx.env).is_none() {
            return style;
        }
        let Some(id) = self.id else {
            return style;
        };
        let value = |property| cx.runtime_state.motion.values.get(&(id, property));

        if let Some(MotionValue::Fill(background)) = value(MotionPropertyId::BackgroundFill) {
            style.background_fill = Some(background.clone());
        }
        if let Some(MotionValue::Color(background)) = value(MotionPropertyId::BackgroundColor) {
            style.background_fill = Some(Fill::Solid(*background));
        }
        if let Some(MotionValue::Color(text_color)) = value(MotionPropertyId::TextColor) {
            style.text_color = *text_color;
        }
        if let Some(MotionValue::Px(radius)) = value(MotionPropertyId::CornerRadius) {
            style.corner_radius = *radius;
        }
        if let Some(MotionValue::Shadows(shadows)) = value(MotionPropertyId::BoxShadows) {
            let (outer, inset) = shadows.iter().copied().partition(|shadow| !shadow.inset);
            style.outer_shadows = outer;
            style.inset_shadows = inset;
        }

        let animated_border_color = match value(MotionPropertyId::BorderColor) {
            Some(MotionValue::Color(color)) => Some(*color),
            _ => None,
        };
        let animated_border_width = match value(MotionPropertyId::BorderWidth) {
            Some(MotionValue::Px(width)) => Some(*width),
            _ => None,
        };
        if animated_border_color.is_some() || animated_border_width.is_some() {
            let mut stroke = style.stroke.unwrap_or_else(|| Stroke {
                fill: Fill::Solid(IrColor::TRANSPARENT),
                width: 0.0,
                dash_array: None,
                line_cap: fission_ir::op::LineCap::Butt,
                line_join: fission_ir::op::LineJoin::Miter,
            });
            if let Some(color) = animated_border_color {
                stroke.fill = Fill::Solid(color);
            }
            if let Some(width) = animated_border_width {
                stroke.width = width;
            }
            style.stroke = Some(stroke);
        }

        style
    }

    fn motion_composite_style(
        &self,
        env: &Env,
        id: WidgetId,
        resolved: &ButtonStyleResolved,
    ) -> CompositeStyle {
        let recipe_styles = self.interaction_styles(env);
        let opacity_varies = recipe_styles.map(|style| style.opacity).varies();
        let translate_y_varies = recipe_styles.map(|style| style.translate_y).varies();
        let recipe_motion = self.recipe_component_motion(env).is_some();
        let mut style = CompositeStyle {
            opacity: (resolved.opacity != 1.0 || opacity_varies).then(|| {
                let scalar = CompositeScalar::new(resolved.opacity);
                if recipe_motion && opacity_varies {
                    scalar.motion(id)
                } else {
                    scalar
                }
            }),
            translate_y: (resolved.translate_y != 0.0 || translate_y_varies).then(|| {
                let scalar = CompositeScalar::new(resolved.translate_y);
                if recipe_motion && translate_y_varies {
                    scalar.motion(id)
                } else {
                    scalar
                }
            }),
            repaint_boundary: recipe_motion || self.motion.is_some(),
            ..Default::default()
        };
        let Some(motion) = &self.motion else {
            return style;
        };
        for track in motion.interaction_tracks(id) {
            if track.phase != MotionPhase::Composite {
                continue;
            }
            match track.property {
                MotionPropertyId::Opacity => {
                    style.opacity = Some(CompositeScalar::new(resolved.opacity).motion(id));
                }
                MotionPropertyId::TranslateX => {
                    style.translate_x = Some(CompositeScalar::new(0.0).motion(id));
                }
                MotionPropertyId::TranslateY => {
                    style.translate_y = Some(CompositeScalar::new(resolved.translate_y).motion(id));
                }
                MotionPropertyId::Scale => {
                    style.scale = Some(CompositeScalar::new(1.0).motion(id));
                }
                MotionPropertyId::Rotation => {
                    style.rotation = Some(CompositeScalar::new(0.0).motion(id));
                }
                _ => {}
            }
        }
        style
    }

    fn should_attach_semantics(&self) -> bool {
        self.semantics.is_some() || self.on_press.is_some() || self.icon_content.is_some()
    }

    fn build_semantics(&self) -> Option<Semantics> {
        if !self.should_attach_semantics() {
            return None;
        }

        let mut semantics = self
            .semantics
            .clone()
            .unwrap_or_else(default_button_semantics);

        semantics.disabled = self.disabled;
        semantics.focus_policy = self.focus_policy;

        if let Some(action_envelope) = &self.on_press {
            if !self.disabled {
                semantics.actions.entries.push(ActionEntry {
                    trigger: fission_ir::semantics::ActionTrigger::Default,
                    action_id: action_envelope.id.as_u128(),
                    payload_data: Some(action_envelope.payload.clone()),
                });
            }
        }

        Some(semantics)
    }
}

fn component_motion_transition(motion: ComponentMotion) -> MotionTransition {
    let easing = match motion.easing {
        EasingCurve::Linear => MotionEasing::Linear,
        EasingCurve::Ease => MotionEasing::CubicBezier(0.25, 0.1, 0.25, 1.0),
        EasingCurve::CubicBezier(x1, y1, x2, y2) => MotionEasing::CubicBezier(x1, y1, x2, y2),
        EasingCurve::Named(name) => match name.trim().to_ascii_lowercase().as_str() {
            "linear" => MotionEasing::Linear,
            "ease" => MotionEasing::CubicBezier(0.25, 0.1, 0.25, 1.0),
            "ease-in" | "ease_in" => MotionEasing::EaseIn,
            "ease-out" | "ease_out" => MotionEasing::EaseOut,
            "ease-in-out" | "ease_in_out" => MotionEasing::EaseInOut,
            _ => MotionEasing::EaseInOut,
        },
    };
    MotionTransition::tween(motion.duration_ms, easing)
}

fn button_paint_track<T: Clone + PartialEq>(
    property: MotionPropertyId,
    values: ButtonStateValues<T>,
    id: WidgetId,
    disabled: bool,
    expression: fn(T) -> MotionExpr,
    transition: MotionTransition,
) -> Option<MotionTrack> {
    button_state_track(
        property,
        MotionPhase::Paint,
        values,
        id,
        disabled,
        expression,
        transition,
    )
}

fn button_composite_track<T: Clone + PartialEq>(
    property: MotionPropertyId,
    values: ButtonStateValues<T>,
    id: WidgetId,
    disabled: bool,
    expression: fn(T) -> MotionExpr,
    transition: MotionTransition,
) -> Option<MotionTrack> {
    button_state_track(
        property,
        MotionPhase::Composite,
        values,
        id,
        disabled,
        expression,
        transition,
    )
}

fn button_state_track<T: Clone + PartialEq>(
    property: MotionPropertyId,
    phase: MotionPhase,
    values: ButtonStateValues<T>,
    id: WidgetId,
    disabled: bool,
    expression: fn(T) -> MotionExpr,
    transition: MotionTransition,
) -> Option<MotionTrack> {
    if !values.varies() {
        return None;
    }
    let start = if disabled {
        values.disabled.clone()
    } else {
        values.rest.clone()
    };
    let from = if crate::build::try_current_runtime_state()
        .is_some_and(|runtime| runtime.motion.values.contains_key(&(id, property.clone())))
    {
        MotionStartValue::Current
    } else {
        MotionStartValue::Explicit(expression(start))
    };
    Some(MotionTrack {
        property,
        phase,
        from,
        to: values.into_motion_expr(id, disabled, expression),
        transition,
    })
}

impl Lower for Button {
    fn lower(&self, cx: &mut LoweringCx) -> WidgetId {
        let mut semantics_op = self.build_semantics();
        if let (Some(semantics), Some(context)) = (&mut semantics_op, cx.form_field_context()) {
            semantics.required |= context.required;
            if let Some(message) = &context.invalid_message {
                semantics.validation_state = TextFieldValidationState::Invalid;
                semantics.validation_message = Some(message.clone());
            }
        }
        if let Some(semantics) = &mut semantics_op {
            if semantics.label.is_none() {
                semantics.label = self
                    .icon_content
                    .as_ref()
                    .map(|content| content.accessible_label.resolve(cx.env))
                    .or_else(|| {
                        self.content
                            .as_ref()
                            .map(|content| content.label.resolve(cx.env))
                    });
            }
        }
        let outermost_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        let (layout_node_id, final_id) = if let Some(_) = semantics_op {
            (cx.next_node_id(), outermost_id)
        } else {
            (outermost_id, outermost_id)
        };

        let resolved_style = self.animated_style(
            cx,
            self.resolve_style(
                cx.env,
                &cx.runtime_state.interaction,
                final_id,
                semantics_op.as_ref().is_some_and(|semantics| {
                    semantics.validation_state == TextFieldValidationState::Invalid
                }),
            ),
        );
        // Recipe padding is logical [start, end, top, bottom]; mirror it once
        // here so no control has to branch on reading order itself.
        let content_padding = self.padding.unwrap_or(match cx.env.layout_direction {
            fission_ir::LayoutDirection::LeftToRight => resolved_style.padding,
            fission_ir::LayoutDirection::RightToLeft => {
                let [start, end, top, bottom] = resolved_style.padding;
                [end, start, top, bottom]
            }
        });
        let layout_padding = [
            content_padding[0] + resolved_style.layout_border_width,
            content_padding[1] + resolved_style.layout_border_width,
            content_padding[2] + resolved_style.layout_border_width,
            content_padding[3] + resolved_style.layout_border_width,
        ];

        cx.push_scope(layout_node_id);

        let mut button_builder = IrBuilder::new(
            layout_node_id,
            Op::Layout(LayoutOp::Box {
                width: self.width.or(resolved_style.width),
                height: self.height,
                min_width: self.min_width,
                max_width: self.max_width.or(resolved_style.max_width),
                min_height: if self.height.is_some() {
                    None
                } else {
                    Some(
                        resolved_style
                            .min_height
                            .unwrap_or(0.0)
                            .max(resolved_style.height),
                    )
                },
                max_height: None,
                padding: layout_padding,
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
                aspect_ratio: None,
            }),
        )
        .composite(self.motion_composite_style(cx.env, final_id, &resolved_style));

        for shadow in &resolved_style.outer_shadows {
            let shadow_id = IrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: None,
                    stroke: None,
                    corner_radius: resolved_style.corner_radius,
                    shadow: Some(*shadow),
                }),
            )
            .build(cx);
            button_builder.add_child(shadow_id);
        }

        let background_id = IrBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: resolved_style.background_fill.clone(),
                stroke: resolved_style.stroke.clone(),
                corner_radius: resolved_style.corner_radius,
                shadow: None,
            }),
        )
        .build(cx);
        button_builder.add_child(background_id);

        for shadow in &resolved_style.inset_shadows {
            let shadow_id = IrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: None,
                    stroke: None,
                    corner_radius: resolved_style.corner_radius,
                    shadow: Some(*shadow),
                }),
            )
            .build(cx);
            button_builder.add_child(shadow_id);
        }

        if let Some(focus_ring) = resolved_style.focus_ring.clone() {
            let focus_ring_id = IrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: None,
                    stroke: Some(focus_ring),
                    corner_radius: resolved_style.corner_radius,
                    shadow: None,
                }),
            )
            .build(cx);
            button_builder.add_child(focus_ring_id);
        }

        let content_id = if let Some(content) = &self.icon_content {
            Some(content.lower_with_style(cx, &resolved_style, final_id))
        } else if let Some(content) = &self.content {
            Some(content.lower_with_style(cx, &resolved_style, final_id))
        } else if let Some(child_widget) = &self.child {
            Some(
                if let Ok(mut text_widget) = child_widget.clone().into_text() {
                    text_widget.color = Some(resolved_style.text_color);
                    text_widget.font_size = Some(resolved_style.font_size);
                    text_widget.font_weight = Some(resolved_style.font_weight);
                    text_widget.line_height = resolved_style.line_height;
                    text_widget.letter_spacing = resolved_style.letter_spacing;
                    text_widget.lower(cx)
                } else {
                    child_widget.lower(cx)
                },
            )
        } else {
            None
        };
        if let Some(child_id) = content_id {
            let aligned_id = match self.content_align {
                ButtonContentAlign::Center => {
                    // Center the content within the button's box (vertically + horizontally).
                    let mut align_builder =
                        IrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Align));
                    align_builder.add_child(child_id);
                    align_builder.build(cx)
                }
                ButtonContentAlign::Start | ButtonContentAlign::End => {
                    let justify = match self.content_align {
                        ButtonContentAlign::Start => fission_ir::op::JustifyContent::Start,
                        ButtonContentAlign::End => fission_ir::op::JustifyContent::End,
                        ButtonContentAlign::Center => fission_ir::op::JustifyContent::Center,
                    };
                    let mut flex_builder = IrBuilder::new(
                        cx.next_node_id(),
                        Op::Layout(LayoutOp::Flex {
                            direction: fission_ir::FlexDirection::Row,
                            wrap: fission_ir::FlexWrap::NoWrap,
                            flex_grow: 1.0,
                            flex_shrink: 0.0,
                            padding: [0.0; 4],
                            gap: None,
                            line_gap: None,
                            align_items: fission_ir::op::AlignItems::Center,
                            justify_content: justify,
                        }),
                    );
                    flex_builder.add_child(child_id);
                    flex_builder.build(cx)
                }
            };
            button_builder.add_child(aligned_id);
        }

        let button_node_id = button_builder.build(cx);

        if let Some(op) = semantics_op {
            let mut semantics_builder = IrBuilder::new(final_id, Op::Semantics(op));
            semantics_builder.add_child(button_node_id);
            let res_id = semantics_builder.build(cx);
            cx.pop_scope();
            return res_id;
        }

        cx.pop_scope();
        button_node_id
    }
}

fn default_button_semantics() -> Semantics {
    Semantics {
        role: Role::Button,
        label: None,
        identifier: None,
        value: None,
        hyperlink: None,
        popover_target: None,
        actions: ActionSet::default(),
        canvas_target: None,
        action_scope_id: None,
        focusable: true,
        sequential_focusable: true,
        focus_policy: FocusPolicy::FocusOnPointer,
        text_editable: false,
        multiline: false,
        text_wrap_mode: fission_ir::semantics::TextWrapMode::Soft,
        masked: false,
        input_mask: None,
        ime_preedit_range: None,
        ime_preedit_cursor_range: None,
        text_selection: None,
        selectable_text: false,
        selection_region: None,
        context_menu: false,
        checked: None,
        selected: None,
        expanded: None,
        has_popup: None,
        orientation: None,
        modal: false,
        controls: Vec::new(),
        labelled_by: Vec::new(),
        described_by: Vec::new(),
        active_descendant: None,
        disabled: false,
        read_only: false,
        autofocus: false,
        draggable: false,
        scrollable_x: false,
        scrollable_y: false,
        min_value: None,
        max_value: None,
        current_value: None,
        is_focus_scope: false,
        is_focus_barrier: false,
        drag_payload: None,
        hero_tag: None,
        focus_index: None,
        text_input_type: fission_ir::semantics::TextInputType::Text,
        text_input_action: fission_ir::semantics::TextInputAction::Done,
        text_capitalization: fission_ir::semantics::TextCapitalization::None,
        max_length: None,
        max_length_enforcement: fission_ir::semantics::MaxLengthEnforcement::Enforced,
        input_formatters: Vec::new(),
        text_field_name: None,
        text_form_id: None,
        autofill_group: None,
        required: false,
        min_length: None,
        validation_pattern: None,
        validation_state: Default::default(),
        validation_message: None,
        autocorrect: true,
        enable_suggestions: true,
        spell_check: true,
        smart_dashes: true,
        smart_quotes: true,
        autofill_hints: Vec::new(),
        scroll_padding: None,
        capture_tab: false,
        auto_indent: false,
    }
}
