use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::motion::{
    color, fill as motion_fill, px, scalar, shadows as motion_shadows, Motion, MotionExpr,
    MotionPredicate, MotionPropertyId, MotionStartValue, MotionTrack, MotionTransition,
    MotionValue, RippleFx, RippleLayer,
};
use crate::ui::Widget;
use crate::ActionEnvelope;
use fission_ir::{
    op::{BoxShadow, BoxStyle, Color, Fill, LayoutOp, Length, Op, PaintOp, Stroke},
    semantics::ActionTrigger,
    ActionEntry, CompositeScalar, CompositeStyle, FocusPolicy, Role, Semantics, WidgetId,
};
use serde::{Deserialize, Serialize};

use super::split_box_margin;

/// Accessibility behavior exposed by a [`Pressable`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressableRole {
    #[default]
    Button,
    Link,
    MenuItem,
}

impl PressableRole {
    fn semantics_role(self) -> Role {
        match self {
            Self::Button => Role::Button,
            Self::Link => Role::Link,
            Self::MenuItem => Role::MenuItem,
        }
    }
}

/// Partial visual style used by a [`Pressable`] interaction state.
///
/// Every property is opt-in. An empty style adds no fill, padding, border,
/// shadow, opacity, scale, or geometry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PressableStyle {
    pub background: Option<Fill>,
    pub border: Option<Stroke>,
    pub corner_radius: Option<f32>,
    pub shadows: Option<Vec<BoxShadow>>,
    pub padding: Option<[Length; 4]>,
    pub opacity: Option<f32>,
    pub scale: Option<f32>,
}

impl PressableStyle {
    fn merged(&self, overlay: Option<&Self>) -> Self {
        let Some(overlay) = overlay else {
            return self.clone();
        };
        Self {
            background: overlay
                .background
                .clone()
                .or_else(|| self.background.clone()),
            border: overlay.border.clone().or_else(|| self.border.clone()),
            corner_radius: overlay.corner_radius.or(self.corner_radius),
            shadows: overlay.shadows.clone().or_else(|| self.shadows.clone()),
            padding: overlay.padding.clone().or_else(|| self.padding.clone()),
            opacity: overlay.opacity.or(self.opacity),
            scale: overlay.scale.or(self.scale),
        }
    }
}

/// A visually neutral accessible interaction surface.
///
/// `Pressable` contributes no visual chrome or geometry unless explicitly
/// supplied through [`PressableStyle`] or [`BoxStyle`]. It still provides
/// semantic activation, focus handling, keyboard activation, pointer state,
/// optional transitions, and optional ripple feedback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pressable {
    pub id: Option<WidgetId>,
    pub child: Widget,
    pub on_press: Option<ActionEnvelope>,
    pub label: Option<String>,
    pub semantics_identifier: Option<String>,
    pub role: PressableRole,
    pub focus_policy: FocusPolicy,
    pub disabled: bool,
    pub layout: BoxStyle,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub style: PressableStyle,
    pub hover_style: Option<PressableStyle>,
    pub pressed_style: Option<PressableStyle>,
    pub focused_style: Option<PressableStyle>,
    pub disabled_style: Option<PressableStyle>,
    pub transition: Option<MotionTransition>,
    pub ripple: Option<RippleFx>,
}

impl Pressable {
    const MOTION_SALT: u32 = 0x5052_4553;

    /// Creates a neutral pressable around `child`.
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            ..Default::default()
        }
    }

    /// Uses an explicit stable identity for interaction, focus, and motion.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Dispatches `action` for pointer, keyboard, or accessibility activation.
    pub fn on_press(mut self, action: ActionEnvelope) -> Self {
        self.on_press = Some(action);
        self
    }

    /// Enables or disables activation and focus participation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the shared typed box model without introducing visual defaults.
    pub fn layout(mut self, style: BoxStyle) -> Self {
        self.layout = style;
        self
    }

    /// Selects how pointer activation affects keyboard focus.
    pub fn focus_policy(mut self, policy: FocusPolicy) -> Self {
        self.focus_policy = policy;
        self
    }

    /// Sets the base visual style.
    pub fn style(mut self, style: PressableStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets the style overlay while the pointer is over the pressable.
    pub fn hover(mut self, style: PressableStyle) -> Self {
        self.hover_style = Some(style);
        self
    }

    /// Sets the style overlay while the pressable is active.
    pub fn pressed(mut self, style: PressableStyle) -> Self {
        self.pressed_style = Some(style);
        self
    }

    /// Sets the style overlay while the pressable owns keyboard focus.
    pub fn focused(mut self, style: PressableStyle) -> Self {
        self.focused_style = Some(style);
        self
    }

    /// Sets the style overlay while activation is disabled.
    pub fn disabled_style(mut self, style: PressableStyle) -> Self {
        self.disabled_style = Some(style);
        self
    }

    /// Animates supported properties between interaction states.
    pub fn transition(mut self, transition: MotionTransition) -> Self {
        self.transition = Some(transition);
        self
    }

    /// Enables optional ripple feedback.
    pub fn ripple(mut self, ripple: RippleFx) -> Self {
        self.ripple = Some(ripple);
        self
    }

    /// Sets a stable semantic identifier for tests and accessibility tools.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    /// Sets the accessible label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets button, link, or menu-item semantics.
    pub fn role(mut self, role: PressableRole) -> Self {
        self.role = role;
        self
    }

    fn resolved_style(&self, cx: &InternalLoweringCx<'_>, id: WidgetId) -> PressableStyle {
        if self.disabled {
            return self.style.merged(self.disabled_style.as_ref());
        }
        if cx.runtime_state.interaction.is_pressed(id) {
            self.style.merged(self.pressed_style.as_ref())
        } else if cx.runtime_state.interaction.is_focused(id) {
            self.style.merged(self.focused_style.as_ref())
        } else if cx.runtime_state.interaction.is_hovered(id) {
            self.style.merged(self.hover_style.as_ref())
        } else {
            self.style.clone()
        }
    }

    fn state_scalar_expr(&self, property: fn(&PressableStyle) -> Option<f32>) -> MotionExpr {
        let base = property(&self.style).unwrap_or(1.0);
        if self.disabled {
            return scalar(
                property(&self.style.merged(self.disabled_style.as_ref())).unwrap_or(base),
            );
        }
        let hover = property(&self.style.merged(self.hover_style.as_ref())).unwrap_or(base);
        let focused = property(&self.style.merged(self.focused_style.as_ref())).unwrap_or(base);
        let pressed = property(&self.style.merged(self.pressed_style.as_ref())).unwrap_or(base);
        let id = self.id.expect("pressable motion requires a stable id");
        MotionExpr::If {
            predicate: MotionPredicate::Pressed(id),
            then_expr: Box::new(scalar(pressed)),
            else_expr: Box::new(MotionExpr::If {
                predicate: MotionPredicate::Focused(id),
                then_expr: Box::new(scalar(focused)),
                else_expr: Box::new(MotionExpr::If {
                    predicate: MotionPredicate::Hovered(id),
                    then_expr: Box::new(scalar(hover)),
                    else_expr: Box::new(scalar(base)),
                }),
            }),
        }
    }

    fn state_px_expr(
        &self,
        property: impl Fn(&PressableStyle) -> Option<f32> + Copy,
    ) -> MotionExpr {
        self.state_value_expr(property, 0.0, px)
    }

    fn state_color_expr(&self, property: fn(&PressableStyle) -> Option<Color>) -> MotionExpr {
        let transparent = Color::TRANSPARENT;
        let base = property(&self.style).unwrap_or(transparent);
        if self.disabled {
            return color(
                property(&self.style.merged(self.disabled_style.as_ref())).unwrap_or(base),
            );
        }
        let hover = property(&self.style.merged(self.hover_style.as_ref())).unwrap_or(base);
        let focused = property(&self.style.merged(self.focused_style.as_ref())).unwrap_or(base);
        let pressed = property(&self.style.merged(self.pressed_style.as_ref())).unwrap_or(base);
        let id = self.id.expect("pressable motion requires a stable id");
        MotionExpr::If {
            predicate: MotionPredicate::Pressed(id),
            then_expr: Box::new(color(pressed)),
            else_expr: Box::new(MotionExpr::If {
                predicate: MotionPredicate::Focused(id),
                then_expr: Box::new(color(focused)),
                else_expr: Box::new(MotionExpr::If {
                    predicate: MotionPredicate::Hovered(id),
                    then_expr: Box::new(color(hover)),
                    else_expr: Box::new(color(base)),
                }),
            }),
        }
    }

    fn state_fill_expr(&self) -> MotionExpr {
        let default = Fill::Solid(Color::TRANSPARENT);
        let base = self.style.background.clone().unwrap_or(default);
        if self.disabled {
            return motion_fill(
                self.style
                    .merged(self.disabled_style.as_ref())
                    .background
                    .unwrap_or(base),
            );
        }
        let hover = self
            .style
            .merged(self.hover_style.as_ref())
            .background
            .unwrap_or_else(|| base.clone());
        let focused = self
            .style
            .merged(self.focused_style.as_ref())
            .background
            .unwrap_or_else(|| base.clone());
        let pressed = self
            .style
            .merged(self.pressed_style.as_ref())
            .background
            .unwrap_or_else(|| base.clone());
        let id = self.id.expect("pressable motion requires a stable id");
        MotionExpr::If {
            predicate: MotionPredicate::Pressed(id),
            then_expr: Box::new(motion_fill(pressed)),
            else_expr: Box::new(MotionExpr::If {
                predicate: MotionPredicate::Focused(id),
                then_expr: Box::new(motion_fill(focused)),
                else_expr: Box::new(MotionExpr::If {
                    predicate: MotionPredicate::Hovered(id),
                    then_expr: Box::new(motion_fill(hover)),
                    else_expr: Box::new(motion_fill(base)),
                }),
            }),
        }
    }

    fn state_shadows_expr(&self) -> MotionExpr {
        let base = self.style.shadows.clone().unwrap_or_default();
        if self.disabled {
            return motion_shadows(
                self.style
                    .merged(self.disabled_style.as_ref())
                    .shadows
                    .unwrap_or(base),
            );
        }
        let hover = self
            .style
            .merged(self.hover_style.as_ref())
            .shadows
            .unwrap_or_else(|| base.clone());
        let focused = self
            .style
            .merged(self.focused_style.as_ref())
            .shadows
            .unwrap_or_else(|| base.clone());
        let pressed = self
            .style
            .merged(self.pressed_style.as_ref())
            .shadows
            .unwrap_or_else(|| base.clone());
        let id = self.id.expect("pressable motion requires a stable id");
        MotionExpr::If {
            predicate: MotionPredicate::Pressed(id),
            then_expr: Box::new(motion_shadows(pressed)),
            else_expr: Box::new(MotionExpr::If {
                predicate: MotionPredicate::Focused(id),
                then_expr: Box::new(motion_shadows(focused)),
                else_expr: Box::new(MotionExpr::If {
                    predicate: MotionPredicate::Hovered(id),
                    then_expr: Box::new(motion_shadows(hover)),
                    else_expr: Box::new(motion_shadows(base)),
                }),
            }),
        }
    }

    fn state_value_expr(
        &self,
        property: impl Fn(&PressableStyle) -> Option<f32> + Copy,
        default: f32,
        expression: fn(f32) -> MotionExpr,
    ) -> MotionExpr {
        let base = property(&self.style).unwrap_or(default);
        if self.disabled {
            return expression(
                property(&self.style.merged(self.disabled_style.as_ref())).unwrap_or(base),
            );
        }
        let hover = property(&self.style.merged(self.hover_style.as_ref())).unwrap_or(base);
        let focused = property(&self.style.merged(self.focused_style.as_ref())).unwrap_or(base);
        let pressed = property(&self.style.merged(self.pressed_style.as_ref())).unwrap_or(base);
        let id = self.id.expect("pressable motion requires a stable id");
        MotionExpr::If {
            predicate: MotionPredicate::Pressed(id),
            then_expr: Box::new(expression(pressed)),
            else_expr: Box::new(MotionExpr::If {
                predicate: MotionPredicate::Focused(id),
                then_expr: Box::new(expression(focused)),
                else_expr: Box::new(MotionExpr::If {
                    predicate: MotionPredicate::Hovered(id),
                    then_expr: Box::new(expression(hover)),
                    else_expr: Box::new(expression(base)),
                }),
            }),
        }
    }

    fn any_state(&self, predicate: impl Fn(&PressableStyle) -> bool) -> bool {
        predicate(&self.style)
            || self.hover_style.as_ref().is_some_and(&predicate)
            || self.pressed_style.as_ref().is_some_and(&predicate)
            || self.focused_style.as_ref().is_some_and(&predicate)
            || self.disabled_style.as_ref().is_some_and(predicate)
    }

    fn motion_id(id: WidgetId) -> WidgetId {
        WidgetId::derived(id.as_u128(), &[Self::MOTION_SALT])
    }

    fn animated_style(
        &self,
        cx: &InternalLoweringCx<'_>,
        id: WidgetId,
        mut style: PressableStyle,
    ) -> PressableStyle {
        if self.transition.is_none() {
            return style;
        }
        let motion_id = Self::motion_id(id);
        let value = |property| cx.runtime_state.motion.values.get(&(motion_id, property));

        if let Some(MotionValue::Color(background)) = value(MotionPropertyId::BackgroundColor) {
            style.background = Some(Fill::Solid(*background));
        }
        if let Some(MotionValue::Fill(background)) = value(MotionPropertyId::BackgroundFill) {
            style.background = Some(background.clone());
        }
        if let Some(MotionValue::Shadows(shadows)) = value(MotionPropertyId::BoxShadows) {
            style.shadows = Some(shadows.clone());
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
            let mut border = style
                .border
                .clone()
                .or_else(|| self.first_border())
                .unwrap_or_else(|| Stroke {
                    fill: Fill::Solid(Color::TRANSPARENT),
                    width: 0.0,
                    dash_array: None,
                    line_cap: fission_ir::op::LineCap::Butt,
                    line_join: fission_ir::op::LineJoin::Miter,
                });
            if let Some(color) = animated_border_color {
                border.fill = Fill::Solid(color);
            }
            if let Some(width) = animated_border_width {
                border.width = width;
            }
            style.border = Some(border);
        }
        if let Some(MotionValue::Px(radius)) = value(MotionPropertyId::CornerRadius) {
            style.corner_radius = Some(*radius);
        }

        let padding_properties = [
            MotionPropertyId::PaddingLeft,
            MotionPropertyId::PaddingRight,
            MotionPropertyId::PaddingTop,
            MotionPropertyId::PaddingBottom,
        ];
        let mut animated_padding = style.padding.clone().unwrap_or_else(|| {
            std::array::from_fn(|index| Length::Points(self.base_padding(index)))
        });
        let mut has_animated_padding = false;
        for (index, property) in padding_properties.into_iter().enumerate() {
            if let Some(MotionValue::Px(padding)) = value(property) {
                animated_padding[index] = Length::Points(*padding);
                has_animated_padding = true;
            }
        }
        if has_animated_padding {
            style.padding = Some(animated_padding);
        }
        style
    }

    fn first_border(&self) -> Option<Stroke> {
        [
            Some(&self.style),
            self.hover_style.as_ref(),
            self.pressed_style.as_ref(),
            self.focused_style.as_ref(),
            self.disabled_style.as_ref(),
        ]
        .into_iter()
        .flatten()
        .find_map(|style| style.border.clone())
    }

    fn base_padding(&self, index: usize) -> f32 {
        self.style
            .padding
            .as_ref()
            .and_then(|padding| length_points(&padding[index]))
            .unwrap_or(0.0)
    }

    fn motion_tracks(&self, transition: &MotionTransition) -> Vec<MotionTrack> {
        let mut tracks = Vec::new();
        if self.style.opacity.is_some()
            || self
                .hover_style
                .as_ref()
                .is_some_and(|style| style.opacity.is_some())
            || self
                .pressed_style
                .as_ref()
                .is_some_and(|style| style.opacity.is_some())
            || self
                .focused_style
                .as_ref()
                .is_some_and(|style| style.opacity.is_some())
            || self
                .disabled_style
                .as_ref()
                .is_some_and(|style| style.opacity.is_some())
        {
            tracks.push(
                MotionTrack::composite(
                    MotionPropertyId::Opacity,
                    MotionStartValue::Explicit(scalar(self.style.opacity.unwrap_or(1.0))),
                    self.state_scalar_expr(|style| style.opacity),
                )
                .transition(transition.clone()),
            );
        }
        if self.style.scale.is_some()
            || self
                .hover_style
                .as_ref()
                .is_some_and(|style| style.scale.is_some())
            || self
                .pressed_style
                .as_ref()
                .is_some_and(|style| style.scale.is_some())
            || self
                .focused_style
                .as_ref()
                .is_some_and(|style| style.scale.is_some())
            || self
                .disabled_style
                .as_ref()
                .is_some_and(|style| style.scale.is_some())
        {
            tracks.push(
                MotionTrack::composite(
                    MotionPropertyId::Scale,
                    MotionStartValue::Explicit(scalar(self.style.scale.unwrap_or(1.0))),
                    self.state_scalar_expr(|style| style.scale),
                )
                .transition(transition.clone()),
            );
        }
        let has_non_solid_background = self.any_state(|style| {
            style
                .background
                .as_ref()
                .is_some_and(|fill| !matches!(fill, Fill::Solid(_)))
        });
        if has_non_solid_background {
            tracks.push(
                MotionTrack::paint(
                    MotionPropertyId::BackgroundFill,
                    MotionStartValue::Explicit(motion_fill(
                        self.style
                            .background
                            .clone()
                            .unwrap_or(Fill::Solid(Color::TRANSPARENT)),
                    )),
                    self.state_fill_expr(),
                )
                .transition(MotionTransition::Instant),
            );
        } else if self.any_state(|style| solid_background(style).is_some()) {
            tracks.push(
                MotionTrack::paint(
                    MotionPropertyId::BackgroundColor,
                    MotionStartValue::Explicit(color(
                        solid_background(&self.style).unwrap_or(Color::TRANSPARENT),
                    )),
                    self.state_color_expr(solid_background),
                )
                .transition(transition.clone()),
            );
        }
        if self.any_state(|style| style.shadows.is_some()) {
            tracks.push(
                MotionTrack::paint(
                    MotionPropertyId::BoxShadows,
                    MotionStartValue::Explicit(motion_shadows(
                        self.style.shadows.clone().unwrap_or_default(),
                    )),
                    self.state_shadows_expr(),
                )
                .transition(MotionTransition::Instant),
            );
        }
        if self.any_state(|style| solid_border_color(style).is_some()) {
            tracks.push(
                MotionTrack::paint(
                    MotionPropertyId::BorderColor,
                    MotionStartValue::Explicit(color(
                        solid_border_color(&self.style).unwrap_or(Color::TRANSPARENT),
                    )),
                    self.state_color_expr(solid_border_color),
                )
                .transition(transition.clone()),
            );
        }
        if self.any_state(|style| style.border.is_some()) {
            tracks.push(
                MotionTrack::paint(
                    MotionPropertyId::BorderWidth,
                    MotionStartValue::Explicit(px(border_width(&self.style).unwrap_or(0.0))),
                    self.state_px_expr(border_width),
                )
                .transition(transition.clone()),
            );
        }
        if self.any_state(|style| style.corner_radius.is_some()) {
            tracks.push(
                MotionTrack::paint(
                    MotionPropertyId::CornerRadius,
                    MotionStartValue::Explicit(px(self.style.corner_radius.unwrap_or(0.0))),
                    self.state_px_expr(|style| style.corner_radius),
                )
                .transition(transition.clone()),
            );
        }
        for (index, property) in [
            MotionPropertyId::PaddingLeft,
            MotionPropertyId::PaddingRight,
            MotionPropertyId::PaddingTop,
            MotionPropertyId::PaddingBottom,
        ]
        .into_iter()
        .enumerate()
        {
            if self.any_state(|style| padding_points(style, index).is_some()) {
                tracks.push(
                    MotionTrack::layout(
                        property,
                        MotionStartValue::Explicit(px(self.base_padding(index))),
                        self.state_value_expr(move |style| padding_points(style, index), 0.0, px),
                    )
                    .transition(transition.clone()),
                );
            }
        }
        tracks
    }
}

fn length_points(length: &Length) -> Option<f32> {
    match length {
        Length::Points(value) => Some(*value),
        _ => None,
    }
}

fn padding_points(style: &PressableStyle, index: usize) -> Option<f32> {
    style
        .padding
        .as_ref()
        .and_then(|padding| length_points(&padding[index]))
}

fn solid_background(style: &PressableStyle) -> Option<Color> {
    match &style.background {
        Some(Fill::Solid(color)) => Some(*color),
        _ => None,
    }
}

fn solid_border_color(style: &PressableStyle) -> Option<Color> {
    match style.border.as_ref().map(|border| &border.fill) {
        Some(Fill::Solid(color)) => Some(*color),
        _ => None,
    }
}

fn border_width(style: &PressableStyle) -> Option<f32> {
    style.border.as_ref().map(|border| border.width)
}

impl Default for Pressable {
    fn default() -> Self {
        Self {
            id: None,
            child: crate::ui::Spacer::default().into(),
            on_press: None,
            label: None,
            semantics_identifier: None,
            role: PressableRole::Button,
            focus_policy: FocusPolicy::FocusOnPointer,
            disabled: false,
            layout: BoxStyle::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            style: PressableStyle::default(),
            hover_style: None,
            pressed_style: None,
            focused_style: None,
            disabled_style: None,
            transition: None,
            ripple: None,
        }
    }
}

impl InternalLower for Pressable {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        let layout_id = cx.next_node_id();
        let style = self.animated_style(cx, id, self.resolved_style(cx, id));
        cx.push_scope(layout_id);

        let mut layout_style = self.layout.clone();
        layout_style.padding = style.padding.clone().or(layout_style.padding);
        let margin_style = split_box_margin(&mut layout_style);
        let position = layout_style.position.take();
        let grid = layout_style.grid.take();
        let flex_grow = layout_style
            .flex_grow
            .map(|value| value.0)
            .unwrap_or(self.flex_grow);
        let flex_shrink = layout_style
            .flex_shrink
            .map(|value| value.0)
            .unwrap_or(self.flex_shrink);
        let mut layout = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::StyledBox {
                style: layout_style,
                flex_grow,
                flex_shrink,
            }),
        )
        .composite(CompositeStyle {
            opacity: self
                .transition
                .is_none()
                .then(|| style.opacity.map(CompositeScalar::new))
                .flatten(),
            scale: self
                .transition
                .is_none()
                .then(|| style.scale.map(CompositeScalar::new))
                .flatten(),
            ..Default::default()
        });

        for shadow in style.shadows.as_deref().unwrap_or_default() {
            layout.add_child(
                InternalIrBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: None,
                        stroke: None,
                        corner_radius: style.corner_radius.unwrap_or(0.0),
                        shadow: Some(*shadow),
                    }),
                )
                .build(cx),
            );
        }
        if style.background.is_some() || style.border.is_some() {
            layout.add_child(
                InternalIrBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: style.background,
                        stroke: style.border,
                        corner_radius: style.corner_radius.unwrap_or(0.0),
                        shadow: None,
                    }),
                )
                .build(cx),
            );
        }
        layout.add_child(self.child.lower(cx));
        let layout_id = layout.build(cx);
        cx.pop_scope();

        let mut semantics = Semantics {
            role: self.role.semantics_role(),
            label: self.label.clone(),
            identifier: self.semantics_identifier.clone(),
            focusable: !self.disabled,
            focus_policy: self.focus_policy,
            disabled: self.disabled,
            ..Default::default()
        };
        if let Some(action) = &self.on_press {
            if !self.disabled {
                semantics.actions.entries.push(ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: action.id.as_u128(),
                    payload_data: Some(action.payload.clone()),
                });
            }
        }
        let mut semantics_node = InternalIrBuilder::new(id, Op::Semantics(semantics));
        semantics_node.add_child(layout_id);
        let mut content_id = semantics_node.build(cx);

        if let Some(margin_style) = margin_style {
            let mut outer = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::StyledBox {
                    style: margin_style,
                    flex_grow,
                    flex_shrink,
                }),
            );
            outer.add_child(content_id);
            content_id = outer.build(cx);
        }
        if let Some(position) = position {
            let mut outer = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::PositionedLengths {
                    left: position.left,
                    top: position.top,
                    right: position.right,
                    bottom: position.bottom,
                    width: None,
                    height: None,
                }),
            );
            outer.add_child(content_id);
            content_id = outer.build(cx);
        }
        if let Some(grid) = grid {
            let mut outer = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::GridItem {
                    row_start: grid.row_start,
                    row_end: grid.row_end,
                    col_start: grid.col_start,
                    col_end: grid.col_end,
                }),
            );
            outer.add_child(content_id);
            content_id = outer.build(cx);
        }

        content_id
    }
}

impl From<Pressable> for Widget {
    fn from(mut pressable: Pressable) -> Self {
        let id = pressable
            .id
            .or_else(crate::build::current_widget_id)
            .or_else(|| crate::build::next_implicit_widget_id(Pressable::MOTION_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission.core.pressable"));
        pressable.id = Some(id);
        let transition = pressable.transition.clone();
        let ripple = (!pressable.disabled)
            .then(|| pressable.ripple.clone())
            .flatten();
        pressable.ripple = None;
        let state_transition = transition.unwrap_or(MotionTransition::Instant);
        let tracks = pressable.motion_tracks(&state_transition);
        let base = Widget::from_pressable_raw(pressable);
        let animated = if tracks.is_empty() {
            base
        } else {
            Motion {
                id: Pressable::motion_id(id),
                tracks,
                child: base,
                ..Default::default()
            }
            .into()
        };
        if let Some(effect) = ripple {
            RippleLayer {
                id: WidgetId::derived(id.as_u128(), &[0x5249_5050]),
                effect,
                child: animated,
            }
            .into()
        } else {
            animated
        }
    }
}
