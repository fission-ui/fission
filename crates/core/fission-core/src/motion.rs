//! Declarative widget motion.
//!
//! Motion is described during widget building and evaluated by the runtime from
//! an explicit clock. Application code declares targets; shells never call back
//! into user code per frame.

use crate::ui::{Composite, Spacer, Widget};
use crate::CurrentTime;
use fission_ir::op::Color;
use fission_ir::{CompositeScalar, WidgetId};
use fission_layout::{LayoutPoint, LayoutSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ops::Add;
use std::sync::Arc;

/// Converts user-facing motion identifiers into stable [`WidgetId`] values.
///
/// This is used by convenience helpers such as [`presence`] and [`appear`] so
/// callers can pass either an explicit [`WidgetId`] or a stable string key.
///
/// ```rust,ignore
/// let card = fission::motion::appear("welcome_card", fission::motion::fade(), child);
/// ```
pub trait IntoMotionId {
    /// Converts this value into the widget identity used by the motion runtime.
    fn into_motion_id(self) -> WidgetId;
}

impl IntoMotionId for WidgetId {
    fn into_motion_id(self) -> WidgetId {
        self
    }
}

impl IntoMotionId for &'static str {
    fn into_motion_id(self) -> WidgetId {
        WidgetId::explicit(self)
    }
}

impl IntoMotionId for String {
    fn into_motion_id(self) -> WidgetId {
        WidgetId::explicit(&self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// The rendering stage affected by a motion track.
///
/// Composite motion is the cheapest path and should be preferred for opacity,
/// translate, scale, and rotation. Layout and paint tracks are declarative
/// inputs for shells that can animate size, position, color, or drawing data.
pub enum MotionPhase {
    /// The track affects layout values such as width or height.
    Layout,
    /// The track affects compositor values such as opacity or transform.
    Composite,
    /// The track affects paint-only values such as color.
    Paint,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// Identifies the property a [`MotionTrack`] animates.
///
/// The property decides the default value, value unit, and shell binding used
/// during rendering. Built-in properties are understood by Fission shells.
/// [`MotionPropertyId::Custom`] is for widget-specific progress values that are
/// consumed by custom renderers rather than by the standard compositor.
///
/// ```rust,ignore
/// use fission::motion::{
///     scalar, MotionPropertyId, MotionStartValue, MotionTrack,
/// };
///
/// let fade = MotionTrack::composite(
///     MotionPropertyId::Opacity,
///     MotionStartValue::Explicit(scalar(0.0)),
///     scalar(1.0),
/// );
/// ```
pub enum MotionPropertyId {
    /// Compositor opacity; scalar where `0.0` is transparent and `1.0` is opaque.
    Opacity,
    /// Horizontal compositor translation in logical pixels.
    TranslateX,
    /// Vertical compositor translation in logical pixels.
    TranslateY,
    /// Uniform compositor scale; scalar where `1.0` is unchanged size.
    Scale,
    /// Compositor rotation in degrees.
    Rotation,
    /// Layout width in logical pixels.
    Width,
    /// Layout height in logical pixels.
    Height,
    /// Absolute layout x-position in logical pixels.
    LayoutX,
    /// Absolute layout y-position in logical pixels.
    LayoutY,
    /// Layout snapshot width in logical pixels.
    LayoutWidth,
    /// Layout snapshot height in logical pixels.
    LayoutHeight,
    /// Intrinsic width of the tracked widget in logical pixels.
    IntrinsicWidth,
    /// Intrinsic height of the tracked widget in logical pixels.
    IntrinsicHeight,
    /// Paint/layout corner radius in logical pixels.
    CornerRadius,
    /// Paint background color.
    BackgroundColor,
    /// Paint border color.
    BorderColor,
    /// Paint text color.
    TextColor,
    /// Widget-defined property consumed by custom renderers or widget code.
    Custom(Arc<str>),
}

impl MotionPropertyId {
    /// Convenience constructor for [`MotionPropertyId::Opacity`].
    pub fn opacity() -> Self {
        Self::Opacity
    }

    /// Convenience constructor for [`MotionPropertyId::TranslateX`].
    pub fn translate_x() -> Self {
        Self::TranslateX
    }

    /// Convenience constructor for [`MotionPropertyId::TranslateY`].
    pub fn translate_y() -> Self {
        Self::TranslateY
    }

    /// Convenience constructor for [`MotionPropertyId::Scale`].
    pub fn scale() -> Self {
        Self::Scale
    }

    /// Convenience constructor for [`MotionPropertyId::Rotation`].
    pub fn rotation() -> Self {
        Self::Rotation
    }

    /// Creates a widget-defined motion property.
    ///
    /// Use a namespaced string such as `"my_crate::chart_progress"` to avoid
    /// collisions with other widgets.
    pub fn custom(name: impl Into<String>) -> Self {
        Self::Custom(Arc::from(name.into()))
    }

    /// Returns the implicit starting value used when no runtime value exists.
    pub fn default_value(&self) -> MotionValue {
        match self {
            Self::Opacity | Self::Scale => MotionValue::Scalar(1.0),
            Self::BackgroundColor | Self::BorderColor | Self::TextColor => {
                MotionValue::Color(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                })
            }
            Self::TranslateX
            | Self::TranslateY
            | Self::Width
            | Self::Height
            | Self::LayoutX
            | Self::LayoutY
            | Self::LayoutWidth
            | Self::LayoutHeight
            | Self::IntrinsicWidth
            | Self::IntrinsicHeight
            | Self::CornerRadius => MotionValue::Px(0.0),
            Self::Rotation => MotionValue::Deg(0.0),
            Self::Custom(_) => MotionValue::Scalar(0.0),
        }
    }

    /// Returns the default as a scalar-like value for compositor resolution.
    ///
    /// Color and boolean defaults resolve to `0.0`.
    pub fn default_scalar_value(&self) -> f32 {
        self.default_value().as_scalar_like().unwrap_or(0.0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A typed value produced by a [`MotionExpr`] or held in [`MotionStateMap`].
///
/// Fission keeps units explicit so a color interpolation cannot accidentally be
/// used as a transform, and pixel values can remain distinguishable from unit
/// scalar progress values.
pub enum MotionValue {
    /// Boolean value for predicates or custom widget values.
    Bool(bool),
    /// Unitless number, commonly used for opacity, scale, or progress.
    Scalar(f32),
    /// Logical pixel value.
    Px(f32),
    /// Angle in degrees.
    Deg(f32),
    /// RGBA color value.
    Color(Color),
}

impl MotionValue {
    /// Returns the contained number for scalar, pixel, or degree values.
    pub fn as_scalar_like(&self) -> Option<f32> {
        match self {
            Self::Scalar(v) | Self::Px(v) | Self::Deg(v) => Some(*v),
            Self::Bool(_) | Self::Color(_) => None,
        }
    }

    fn interpolate(&self, to: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        match (self, to) {
            (Self::Scalar(a), Self::Scalar(b)) => Self::Scalar(lerp(*a, *b, t)),
            (Self::Px(a), Self::Px(b)) => Self::Px(lerp(*a, *b, t)),
            (Self::Deg(a), Self::Deg(b)) => Self::Deg(lerp(*a, *b, t)),
            (Self::Color(a), Self::Color(b)) => Self::Color(Color {
                r: lerp(a.r as f32, b.r as f32, t).round().clamp(0.0, 255.0) as u8,
                g: lerp(a.g as f32, b.g as f32, t).round().clamp(0.0, 255.0) as u8,
                b: lerp(a.b as f32, b.b as f32, t).round().clamp(0.0, 255.0) as u8,
                a: lerp(a.a as f32, b.a as f32, t).round().clamp(0.0, 255.0) as u8,
            }),
            _ => {
                if t >= 1.0 {
                    to.clone()
                } else {
                    self.clone()
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// Runtime interaction predicate that can branch inside a [`MotionExpr`].
///
/// Predicates are evaluated by the shell from deterministic runtime state, not
/// by calling user code every frame.
pub enum MotionPredicate {
    /// True when the widget is currently hovered.
    Hovered(WidgetId),
    /// True when the widget is currently pressed.
    Pressed(WidgetId),
    /// True when the widget is currently focused.
    Focused(WidgetId),
    /// True when the widget is disabled.
    Disabled(WidgetId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Declarative expression evaluated by the motion runtime.
///
/// Expressions are the closed IR for motion targets. They can reference layout
/// snapshots, pointer position, interaction predicates, and numeric operators
/// without requiring arbitrary per-frame callbacks.
///
/// ```rust,ignore
/// use fission::motion::{
///     scalar, MotionExpr, MotionPredicate, MotionPropertyId, MotionStartValue, MotionTrack,
/// };
/// use fission::WidgetId;
///
/// let button_id = WidgetId::explicit("save_button");
/// let scale = MotionTrack::composite(
///     MotionPropertyId::Scale,
///     MotionStartValue::Current,
///     MotionExpr::If {
///         predicate: MotionPredicate::Pressed(button_id),
///         then_expr: Box::new(scalar(0.96)),
///         else_expr: Box::new(scalar(1.0)),
///     },
/// );
/// ```
pub enum MotionExpr {
    /// Literal typed value.
    Value(MotionValue),
    /// Current widget intrinsic width.
    IntrinsicWidth,
    /// Current widget intrinsic height.
    IntrinsicHeight,
    /// X position of another widget from the latest layout snapshot.
    LayoutX(WidgetId),
    /// Y position of another widget from the latest layout snapshot.
    LayoutY(WidgetId),
    /// Width of another widget from the latest layout snapshot.
    LayoutWidth(WidgetId),
    /// Height of another widget from the latest layout snapshot.
    LayoutHeight(WidgetId),
    /// Pointer x-position local to the tracked widget.
    PointerLocalX,
    /// Pointer y-position local to the tracked widget.
    PointerLocalY,
    /// Conditional expression selected by a runtime predicate.
    If {
        /// Runtime predicate to test.
        predicate: MotionPredicate,
        /// Expression evaluated when the predicate is true.
        then_expr: Box<MotionExpr>,
        /// Expression evaluated when the predicate is false.
        else_expr: Box<MotionExpr>,
    },
    /// Numeric addition.
    Add(Box<MotionExpr>, Box<MotionExpr>),
    /// Numeric subtraction.
    Sub(Box<MotionExpr>, Box<MotionExpr>),
    /// Numeric multiplication.
    Mul(Box<MotionExpr>, Box<MotionExpr>),
    /// Numeric division. Division by zero leaves the left value unchanged.
    Div(Box<MotionExpr>, Box<MotionExpr>),
    /// Numeric negation.
    Neg(Box<MotionExpr>),
    /// Absolute value.
    Abs(Box<MotionExpr>),
    /// Minimum of two scalar-like expressions.
    Min(Box<MotionExpr>, Box<MotionExpr>),
    /// Maximum of two scalar-like expressions.
    Max(Box<MotionExpr>, Box<MotionExpr>),
    /// Clamps a scalar-like expression between `min` and `max`.
    Clamp {
        /// Value to clamp.
        value: Box<MotionExpr>,
        /// Minimum allowed value.
        min: Box<MotionExpr>,
        /// Maximum allowed value.
        max: Box<MotionExpr>,
    },
    /// Interpolates between two values using scalar-like `t`.
    Lerp {
        /// Start expression.
        from: Box<MotionExpr>,
        /// End expression.
        to: Box<MotionExpr>,
        /// Interpolation progress, normally `0.0..=1.0`.
        t: Box<MotionExpr>,
    },
    /// Maps a scalar-like expression from one range to another.
    MapRange {
        /// Source expression.
        value: Box<MotionExpr>,
        /// Lower bound of the input range.
        from_start: f32,
        /// Upper bound of the input range.
        from_end: f32,
        /// Lower bound of the output range.
        to_start: f32,
        /// Upper bound of the output range.
        to_end: f32,
        /// Whether to clamp the mapped progress to `0.0..=1.0`.
        clamp: bool,
    },
}

impl MotionExpr {
    /// Evaluates the expression against runtime and layout inputs.
    ///
    /// Application code usually does not call this directly; shells and tests
    /// use it to turn declarative targets into concrete [`MotionValue`]s.
    pub fn eval(&self, input: &MotionEvalInput<'_>) -> MotionValue {
        match self {
            Self::Value(value) => value.clone(),
            Self::IntrinsicWidth => input
                .self_rect
                .map(|rect| MotionValue::Px(rect.width()))
                .unwrap_or(MotionValue::Px(0.0)),
            Self::IntrinsicHeight => input
                .self_rect
                .map(|rect| MotionValue::Px(rect.height()))
                .unwrap_or(MotionValue::Px(0.0)),
            Self::LayoutX(id) => input
                .layout
                .and_then(|layout| layout.get_node_rect(*id))
                .map(|rect| MotionValue::Px(rect.x()))
                .unwrap_or(MotionValue::Px(0.0)),
            Self::LayoutY(id) => input
                .layout
                .and_then(|layout| layout.get_node_rect(*id))
                .map(|rect| MotionValue::Px(rect.y()))
                .unwrap_or(MotionValue::Px(0.0)),
            Self::LayoutWidth(id) => input
                .layout
                .and_then(|layout| layout.get_node_rect(*id))
                .map(|rect| MotionValue::Px(rect.width()))
                .unwrap_or(MotionValue::Px(0.0)),
            Self::LayoutHeight(id) => input
                .layout
                .and_then(|layout| layout.get_node_rect(*id))
                .map(|rect| MotionValue::Px(rect.height()))
                .unwrap_or(MotionValue::Px(0.0)),
            Self::PointerLocalX => input
                .pointer_local
                .map(|point| MotionValue::Px(point.x))
                .unwrap_or(MotionValue::Px(0.0)),
            Self::PointerLocalY => input
                .pointer_local
                .map(|point| MotionValue::Px(point.y))
                .unwrap_or(MotionValue::Px(0.0)),
            Self::If {
                predicate,
                then_expr,
                else_expr,
            } => {
                if input.predicate(predicate) {
                    then_expr.eval(input)
                } else {
                    else_expr.eval(input)
                }
            }
            Self::Add(a, b) => numeric_binary(a, b, input, |a, b| a + b),
            Self::Sub(a, b) => numeric_binary(a, b, input, |a, b| a - b),
            Self::Mul(a, b) => numeric_binary(a, b, input, |a, b| a * b),
            Self::Div(a, b) => numeric_binary(a, b, input, |a, b| if b == 0.0 { a } else { a / b }),
            Self::Neg(v) => numeric_unary(v, input, |v| -v),
            Self::Abs(v) => numeric_unary(v, input, f32::abs),
            Self::Min(a, b) => numeric_binary(a, b, input, f32::min),
            Self::Max(a, b) => numeric_binary(a, b, input, f32::max),
            Self::Clamp { value, min, max } => {
                let value = value.eval(input);
                let min = min.eval(input).as_scalar_like().unwrap_or(0.0);
                let max = max.eval(input).as_scalar_like().unwrap_or(min);
                map_numeric(value, |v| v.clamp(min, max))
            }
            Self::Lerp { from, to, t } => {
                let t = t.eval(input).as_scalar_like().unwrap_or(0.0);
                from.eval(input).interpolate(&to.eval(input), t)
            }
            Self::MapRange {
                value,
                from_start,
                from_end,
                to_start,
                to_end,
                clamp,
            } => {
                let raw = value.eval(input).as_scalar_like().unwrap_or(0.0);
                let denom = from_end - from_start;
                let mut t = if denom.abs() <= f32::EPSILON {
                    0.0
                } else {
                    (raw - from_start) / denom
                };
                if *clamp {
                    t = t.clamp(0.0, 1.0);
                }
                MotionValue::Scalar(lerp(*to_start, *to_end, t))
            }
        }
    }
}

#[derive(Clone, Debug)]
/// Inputs used when evaluating a [`MotionExpr`].
///
/// This is primarily shell/runtime API. App code usually declares expressions
/// and lets Fission evaluate them.
pub struct MotionEvalInput<'a> {
    /// Runtime state used for interaction predicates and current values.
    pub runtime: &'a crate::RuntimeState,
    /// Optional layout snapshot used for layout-aware expressions.
    pub layout: Option<&'a LayoutSnapshot>,
    /// Widget currently being evaluated.
    pub self_id: WidgetId,
    /// Layout rect of `self_id`, if known.
    pub self_rect: Option<fission_layout::LayoutRect>,
    /// Latest pointer position in the widget's local coordinate space.
    pub pointer_local: Option<LayoutPoint>,
}

impl<'a> MotionEvalInput<'a> {
    fn predicate(&self, predicate: &MotionPredicate) -> bool {
        match predicate {
            MotionPredicate::Hovered(id) => self.runtime.interaction.is_hovered(*id),
            MotionPredicate::Pressed(id) => self.runtime.interaction.is_pressed(*id),
            MotionPredicate::Focused(id) => self.runtime.interaction.is_focused(*id),
            MotionPredicate::Disabled(_) => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Defines where a motion track starts from when a new target is synchronized.
pub enum MotionStartValue {
    /// Start from the current runtime value, or the property's default value.
    Current,
    /// Start from a concrete expression evaluated at synchronization time.
    Explicit(MotionExpr),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Easing curve applied to tween progress.
///
/// ```rust,ignore
/// let transition = fission::motion::MotionTransition::tween(
///     180,
///     fission::motion::MotionEasing::EaseOut,
/// );
/// ```
pub enum MotionEasing {
    /// No easing; progress is linear.
    Linear,
    /// Slow start, fast end.
    EaseIn,
    /// Fast start, slow end.
    EaseOut,
    /// Slow start and end.
    EaseInOut,
    /// Cubic Bezier curve represented by `(x1, y1, x2, y2)`.
    CubicBezier(f32, f32, f32, f32),
}

impl Default for MotionEasing {
    fn default() -> Self {
        Self::EaseInOut
    }
}

impl MotionEasing {
    /// Applies the easing curve to normalized progress.
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::CubicBezier(_x1, y1, _x2, y2) => {
                let t2 = t * t;
                let t3 = t2 * t;
                3.0 * (1.0 - t) * (1.0 - t) * t * y1 + 3.0 * (1.0 - t) * t2 * y2 + t3
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Timing model for a [`MotionTrack`].
///
/// Use [`MotionTransition::tween`] for fixed-duration motion and
/// [`MotionTransition::spring`] for spring-like motion. Transitions are data,
/// so shells and tests can step them deterministically.
pub enum MotionTransition {
    /// Applies the target value immediately.
    Instant,
    /// Fixed-duration transition.
    Tween {
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Delay before the transition starts, in milliseconds.
        delay_ms: u64,
        /// Easing curve used for tween progress.
        easing: MotionEasing,
        /// Whether the tween repeats indefinitely.
        repeat: bool,
        /// Optional frame interval hint for repeated low-priority motion.
        frame_interval_ms: Option<u64>,
    },
    /// Spring-like transition.
    Spring {
        /// Spring stiffness.
        stiffness: f32,
        /// Spring damping.
        damping: f32,
        /// Spring mass.
        mass: f32,
        /// Completion epsilon.
        epsilon: f32,
        /// Delay before the spring starts, in milliseconds.
        delay_ms: u64,
    },
}

impl Default for MotionTransition {
    fn default() -> Self {
        Self::Tween {
            duration_ms: 160,
            delay_ms: 0,
            easing: MotionEasing::EaseInOut,
            repeat: false,
            frame_interval_ms: None,
        }
    }
}

impl MotionTransition {
    /// Creates a fixed-duration tween transition.
    pub fn tween(duration_ms: u64, easing: MotionEasing) -> Self {
        Self::Tween {
            duration_ms,
            delay_ms: 0,
            easing,
            repeat: false,
            frame_interval_ms: None,
        }
    }

    /// Creates a spring-like transition.
    pub fn spring(stiffness: f32, damping: f32) -> Self {
        Self::Spring {
            stiffness,
            damping,
            mass: 1.0,
            epsilon: 0.001,
            delay_ms: 0,
        }
    }

    /// Sets a start delay in milliseconds.
    pub fn delay_ms(mut self, delay_ms: u64) -> Self {
        match &mut self {
            Self::Instant => {}
            Self::Tween {
                delay_ms: delay, ..
            }
            | Self::Spring {
                delay_ms: delay, ..
            } => {
                *delay = delay_ms;
            }
        }
        self
    }

    /// Enables or disables repeating tween playback.
    pub fn repeat(mut self, repeat: bool) -> Self {
        if let Self::Tween {
            repeat: current, ..
        } = &mut self
        {
            *current = repeat;
        }
        self
    }

    /// Sets an optional frame interval hint for repeated tween playback.
    pub fn frame_interval_ms(mut self, frame_interval_ms: Option<u64>) -> Self {
        if let Self::Tween {
            frame_interval_ms: current,
            ..
        } = &mut self
        {
            *current = frame_interval_ms;
        }
        self
    }

    fn duration_ms(&self) -> u64 {
        match self {
            Self::Instant => 0,
            Self::Tween { duration_ms, .. } => *duration_ms,
            Self::Spring { .. } => 260,
        }
    }

    fn delay_value_ms(&self) -> u64 {
        match self {
            Self::Instant => 0,
            Self::Tween { delay_ms, .. } | Self::Spring { delay_ms, .. } => *delay_ms,
        }
    }

    fn repeat_enabled(&self) -> bool {
        matches!(self, Self::Tween { repeat: true, .. })
    }

    fn easing(&self) -> MotionEasing {
        match self {
            Self::Instant | Self::Spring { .. } => MotionEasing::EaseOut,
            Self::Tween { easing, .. } => easing.clone(),
        }
    }

    fn frame_interval_value_ms(&self) -> Option<u64> {
        match self {
            Self::Tween {
                frame_interval_ms, ..
            } => frame_interval_ms.filter(|ms| *ms > 0),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A single declarative animation from a start value to a target expression.
///
/// Tracks are grouped by [`Motion`] or [`Presence`] and synchronized into the
/// runtime. Later tracks targeting the same property and phase win when widget
/// presets are composed.
///
/// ```rust,ignore
/// use fission::motion::{
///     px, MotionPropertyId, MotionStartValue, MotionTrack, MotionTransition, MotionEasing,
/// };
///
/// let slide = MotionTrack::composite(
///     MotionPropertyId::TranslateY,
///     MotionStartValue::Explicit(px(16.0)),
///     px(0.0),
/// )
/// .transition(MotionTransition::tween(160, MotionEasing::EaseOut));
/// ```
pub struct MotionTrack {
    /// Property affected by this track.
    pub property: MotionPropertyId,
    /// Rendering phase affected by this track.
    pub phase: MotionPhase,
    /// Source value for a newly synchronized target.
    pub from: MotionStartValue,
    /// Target expression evaluated by the runtime.
    pub to: MotionExpr,
    /// Timing model for this track.
    pub transition: MotionTransition,
}

impl MotionTrack {
    /// Creates a compositor-phase track for transform or opacity properties.
    pub fn composite(property: MotionPropertyId, from: MotionStartValue, to: MotionExpr) -> Self {
        Self {
            property,
            phase: MotionPhase::Composite,
            from,
            to,
            transition: MotionTransition::default(),
        }
    }

    /// Replaces the track's timing model.
    pub fn transition(mut self, transition: MotionTransition) -> Self {
        self.transition = transition;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Runtime lifecycle phase for a [`Presence`] widget.
pub enum PresencePhase {
    /// The child is not visible and is not kept mounted.
    Hidden,
    /// The child is mounted and running enter tracks.
    Entering,
    /// The child is mounted and fully visible.
    Present,
    /// The child is mounted and running exit tracks.
    Exiting,
}

impl Default for PresencePhase {
    fn default() -> Self {
        Self::Hidden
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Placement of a ripple layer relative to the wrapped child.
pub enum RipplePlacement {
    /// Draw ripples behind the child content.
    BehindChild,
    /// Draw ripples above the child content.
    AboveChild,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Pointer-origin ripple effect configuration.
///
/// Use [`ripple_effect`] for the default and chain builder methods to tune it.
///
/// ```rust,ignore
/// let ripple = fission::motion::ripple_effect()
///     .scale(12.0)
///     .duration(500)
///     .ease(fission::motion::MotionEasing::EaseOut);
/// ```
pub struct RippleFx {
    /// Ripple color before opacity is applied.
    pub color: Color,
    /// Peak ripple opacity.
    pub opacity: f32,
    /// Final ripple scale relative to the starting circle.
    pub scale: f32,
    /// Timing used for each spawned ripple.
    pub transition: MotionTransition,
    /// Maximum number of ripples kept for one layer.
    pub max_instances: usize,
    /// Whether ripples draw above or behind the child.
    pub placement: RipplePlacement,
}

impl Default for RippleFx {
    fn default() -> Self {
        Self {
            color: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 64,
            },
            opacity: 0.35,
            scale: 10.0,
            transition: MotionTransition::tween(600, MotionEasing::EaseOut),
            max_instances: 8,
            placement: RipplePlacement::BehindChild,
        }
    }
}

impl RippleFx {
    /// Sets the final ripple scale.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the ripple duration in milliseconds when the transition is a tween.
    pub fn duration(mut self, duration_ms: u64) -> Self {
        if let MotionTransition::Tween {
            duration_ms: current,
            ..
        } = &mut self.transition
        {
            *current = duration_ms;
        }
        self
    }

    /// Sets the ripple easing when the transition is a tween.
    pub fn ease(mut self, easing: MotionEasing) -> Self {
        if let MotionTransition::Tween {
            easing: current, ..
        } = &mut self.transition
        {
            *current = easing;
        }
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Runtime declaration emitted by motion widgets during build.
///
/// Shells consume declarations after a build and synchronize them into
/// [`MotionStateMap`]. Application code normally constructs [`Motion`],
/// [`Presence`], or [`RippleLayer`] instead of creating declarations directly.
pub struct MotionDeclaration {
    /// Stable identity for the tracked widget or motion slot.
    pub id: WidgetId,
    /// Declaration payload.
    pub kind: MotionDeclarationKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Payload for a [`MotionDeclaration`].
pub enum MotionDeclarationKind {
    /// Standard set of tracks applied to the declaration id.
    Tracks {
        /// Tracks to synchronize.
        tracks: Vec<MotionTrack>,
    },
    /// Presence lifecycle declaration for enter and exit motion.
    Presence {
        /// Whether the child should be visible in this build.
        visible: bool,
        /// Whether the child remains mounted even after exit completes.
        keep_rendered: bool,
        /// Tracks used when becoming visible.
        enter: Vec<MotionTrack>,
        /// Tracks used when becoming hidden.
        exit: Vec<MotionTrack>,
        /// Whether exiting content should be treated as inert by shells.
        inert_while_exiting: bool,
    },
    /// Pointer-origin ripple layer declaration.
    RippleLayer(RippleFx),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Native motion wrapper for app-owned or framework-owned widgets.
///
/// `Motion` applies tracks to its child and emits one [`MotionDeclaration`].
/// It is the low-level escape hatch under all widget-owned motion presets.
///
/// ```rust,ignore
/// use fission::motion::{fade, Motion};
/// use fission::{Text, WidgetId};
///
/// let child = Text::new("Hello").into();
/// let widget = Motion {
///     id: WidgetId::explicit("hello_fade"),
///     tracks: fade(),
///     child,
///     ..Default::default()
/// };
/// ```
pub struct Motion {
    /// Stable identity for the motion wrapper.
    pub id: WidgetId,
    /// Tracks applied to the child.
    pub tracks: Vec<MotionTrack>,
    /// Wrapped child widget.
    pub child: Widget,
    /// Whether the wrapper clips visual overflow.
    pub clip_to_bounds: bool,
    /// Whether shells should treat this as a repaint/composite boundary.
    pub repaint_boundary: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("motion"),
            tracks: Vec::new(),
            child: Spacer::default().into(),
            clip_to_bounds: false,
            repaint_boundary: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Presence wrapper that keeps a child mounted while enter/exit tracks run.
///
/// Use this when a widget must animate out after its `visible` state becomes
/// false. If both enter and exit tracks are empty, presence changes are applied
/// immediately.
///
/// ```rust,ignore
/// use fission::motion::{fade, Presence};
/// use fission::{Text, WidgetId};
///
/// let widget = Presence {
///     id: WidgetId::explicit("details_panel"),
///     visible: state.show_details,
///     enter: fade(),
///     exit: fission::motion::reverse_tracks_for_exit(&fade()),
///     child: Text::new("Details").into(),
///     ..Default::default()
/// };
/// ```
pub struct Presence {
    /// Stable identity for the presence slot.
    pub id: WidgetId,
    /// Whether the child is logically visible in the current build.
    pub visible: bool,
    /// Whether to keep rendering the child after exit completes.
    pub keep_rendered: bool,
    /// Tracks used when entering.
    pub enter: Vec<MotionTrack>,
    /// Tracks used when exiting.
    pub exit: Vec<MotionTrack>,
    /// Child controlled by this presence wrapper.
    pub child: Widget,
    /// Whether to clip visual overflow during presence motion.
    pub clip_to_bounds: bool,
    /// Whether shells should treat this as a repaint/composite boundary.
    pub repaint_boundary: bool,
    /// Whether shells should suppress interaction while the child exits.
    pub inert_while_exiting: bool,
}

impl Default for Presence {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("presence"),
            visible: true,
            keep_rendered: false,
            enter: Vec::new(),
            exit: Vec::new(),
            child: Spacer::default().into(),
            clip_to_bounds: false,
            repaint_boundary: true,
            inert_while_exiting: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Wrapper that enables deterministic pointer-origin ripple effects.
pub struct RippleLayer {
    /// Stable identity for the ripple layer.
    pub id: WidgetId,
    /// Ripple configuration.
    pub effect: RippleFx,
    /// Wrapped child widget.
    pub child: Widget,
}

impl Default for RippleLayer {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("ripple_layer"),
            effect: RippleFx::default(),
            child: Spacer::default().into(),
        }
    }
}

impl From<Motion> for Widget {
    fn from(component: Motion) -> Self {
        crate::build::try_register_motion(MotionDeclaration {
            id: component.id,
            kind: MotionDeclarationKind::Tracks {
                tracks: component.tracks.clone(),
            },
        });
        let style = composite_style_for_tracks(component.id, component.child, &component.tracks)
            .clip_to_bounds(component.clip_to_bounds)
            .repaint_boundary(component.repaint_boundary);
        style.into()
    }
}

impl From<Presence> for Widget {
    fn from(component: Presence) -> Self {
        crate::build::try_register_motion(MotionDeclaration {
            id: component.id,
            kind: MotionDeclarationKind::Presence {
                visible: component.visible,
                keep_rendered: component.keep_rendered,
                enter: component.enter.clone(),
                exit: component.exit.clone(),
                inert_while_exiting: component.inert_while_exiting,
            },
        });

        let phase = crate::build::try_current_runtime_state()
            .and_then(|runtime| runtime.motion.presence.get(&component.id).copied())
            .unwrap_or(if component.visible {
                PresencePhase::Present
            } else {
                PresencePhase::Hidden
            });
        let should_render = component.visible
            || component.keep_rendered
            || matches!(
                phase,
                PresencePhase::Entering | PresencePhase::Present | PresencePhase::Exiting
            );
        if !should_render {
            return Spacer::default().into();
        }

        let tracks = if component.visible {
            &component.enter
        } else {
            &component.exit
        };
        composite_style_for_tracks(component.id, component.child, tracks)
            .clip_to_bounds(component.clip_to_bounds)
            .repaint_boundary(component.repaint_boundary)
            .into()
    }
}

impl From<RippleLayer> for Widget {
    fn from(component: RippleLayer) -> Self {
        crate::build::try_register_motion(MotionDeclaration {
            id: component.id,
            kind: MotionDeclarationKind::RippleLayer(component.effect),
        });
        Composite {
            id: Some(component.id),
            child: component.child,
            ..Default::default()
        }
        .into()
    }
}

fn composite_style_for_tracks(id: WidgetId, child: Widget, tracks: &[MotionTrack]) -> Composite {
    let mut composite = Composite {
        id: Some(id),
        child,
        ..Default::default()
    };
    for track in tracks {
        if track.phase != MotionPhase::Composite {
            continue;
        }
        match track.property {
            MotionPropertyId::Opacity => {
                composite.style.opacity = Some(CompositeScalar::new(1.0).motion(id));
            }
            MotionPropertyId::TranslateX => {
                composite.style.translate_x = Some(CompositeScalar::new(0.0).motion(id));
            }
            MotionPropertyId::TranslateY => {
                composite.style.translate_y = Some(CompositeScalar::new(0.0).motion(id));
            }
            MotionPropertyId::Scale => {
                composite.style.scale = Some(CompositeScalar::new(1.0).motion(id));
            }
            MotionPropertyId::Rotation => {
                composite.style.rotation = Some(CompositeScalar::new(0.0).motion(id));
            }
            _ => {}
        }
    }
    composite
}

#[derive(Clone, Debug, Default)]
/// Runtime storage for active motion values.
///
/// Shells own this through [`crate::RuntimeState`]. Tests and custom renderers
/// may inspect it to read deterministic motion progress.
pub struct MotionStateMap {
    /// Last known value for each `(widget, property)` pair.
    pub values: HashMap<(WidgetId, MotionPropertyId), MotionValue>,
    /// Active transitions keyed by `(widget, property)`.
    pub active: HashMap<(WidgetId, MotionPropertyId), ActiveMotion>,
    /// Current presence phase for each presence id.
    pub presence: HashMap<WidgetId, PresencePhase>,
    /// Active ripple instances for each ripple layer.
    pub ripples: HashMap<WidgetId, Vec<SpawnedRipple>>,
}

impl MotionStateMap {
    /// Returns a scalar-like value for a widget/property pair.
    ///
    /// If no value has been produced yet, the property's default value is used.
    pub fn scalar_value(&self, widget_id: WidgetId, property: MotionPropertyId) -> f32 {
        self.values
            .get(&(widget_id, property.clone()))
            .and_then(MotionValue::as_scalar_like)
            .unwrap_or_else(|| property.default_scalar_value())
    }
}

#[derive(Clone, Debug)]
/// Runtime transition currently being advanced by the clock.
pub struct ActiveMotion {
    /// Target widget or motion slot.
    pub target: WidgetId,
    /// Property being animated.
    pub property: MotionPropertyId,
    /// Concrete start value.
    pub start_value: MotionValue,
    /// Concrete target value.
    pub end_value: MotionValue,
    /// Runtime start time in milliseconds.
    pub start_time: u64,
    /// Runtime duration in milliseconds.
    pub duration: u64,
    /// Whether this motion repeats.
    pub repeat: bool,
    /// Optional repeated-frame interval hint.
    pub frame_interval_ms: Option<u64>,
    /// Easing used to map progress.
    pub easing: MotionEasing,
}

#[derive(Clone, Debug)]
/// A ripple instance spawned by pointer input.
pub struct SpawnedRipple {
    /// Stable identity for this ripple instance.
    pub id: WidgetId,
    /// Ripple layer that owns this instance.
    pub parent: WidgetId,
    /// Monotonic sequence number within the parent layer.
    pub sequence: u64,
    /// Ripple origin x-position in local pixels.
    pub origin_x: f32,
    /// Ripple origin y-position in local pixels.
    pub origin_y: f32,
    /// Creation time in milliseconds.
    pub birth_ms: u64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

#[derive(Default)]
/// Result of synchronizing declarations into [`MotionStateMap`].
pub struct MotionSyncResult {
    /// Widget/property keys that changed during synchronization.
    pub changed: Vec<(WidgetId, MotionPropertyId)>,
}

/// Synchronizes build-time motion declarations into runtime state.
///
/// This is shell/runtime API. Application code should use widgets and motion
/// presets rather than calling it directly.
pub fn sync_motion_declarations(
    state: &mut MotionStateMap,
    declarations: &[MotionDeclaration],
    runtime: &crate::RuntimeState,
    layout: Option<&LayoutSnapshot>,
    now: CurrentTime,
) -> MotionSyncResult {
    let mut result = MotionSyncResult::default();
    let mut requested = HashSet::new();

    for declaration in declarations {
        match &declaration.kind {
            MotionDeclarationKind::Tracks { tracks } => {
                sync_tracks(
                    state,
                    declaration.id,
                    tracks,
                    runtime,
                    layout,
                    now,
                    &mut requested,
                    &mut result,
                );
            }
            MotionDeclarationKind::Presence {
                visible,
                keep_rendered: _,
                enter,
                exit,
                inert_while_exiting: _,
            } => {
                let phase = state
                    .presence
                    .get(&declaration.id)
                    .copied()
                    .unwrap_or(PresencePhase::Hidden);
                let next_phase = match (phase, *visible) {
                    (PresencePhase::Hidden, true) => PresencePhase::Entering,
                    (PresencePhase::Exiting, true) => PresencePhase::Entering,
                    (PresencePhase::Entering, true) => PresencePhase::Entering,
                    (PresencePhase::Present, true) => PresencePhase::Present,
                    (PresencePhase::Hidden, false) => PresencePhase::Hidden,
                    (PresencePhase::Entering, false)
                    | (PresencePhase::Present, false)
                    | (PresencePhase::Exiting, false) => PresencePhase::Exiting,
                };
                state.presence.insert(declaration.id, next_phase);
                let tracks = if *visible { enter } else { exit };
                if tracks.is_empty() {
                    match next_phase {
                        PresencePhase::Entering => {
                            state
                                .presence
                                .insert(declaration.id, PresencePhase::Present);
                        }
                        PresencePhase::Exiting => {
                            state.presence.insert(declaration.id, PresencePhase::Hidden);
                        }
                        PresencePhase::Hidden | PresencePhase::Present => {}
                    }
                    continue;
                }
                if !*visible && phase == PresencePhase::Hidden {
                    continue;
                }
                sync_tracks(
                    state,
                    declaration.id,
                    tracks,
                    runtime,
                    layout,
                    now,
                    &mut requested,
                    &mut result,
                );
            }
            MotionDeclarationKind::RippleLayer(_) => {}
        }
    }

    state.active.retain(|key, _| requested.contains(key));
    state.values.retain(|key, _| requested.contains(key));
    result
}

/// Advances active motion to `current_time` and returns changed properties.
///
/// This is shell/runtime API used by render loops and deterministic tests.
pub fn tick_motion(
    state: &mut MotionStateMap,
    current_time: CurrentTime,
) -> Vec<(WidgetId, MotionPropertyId)> {
    let mut changed = Vec::new();
    let mut finished = Vec::new();
    let mut finished_presence = Vec::new();

    for ((target, property), motion) in state.active.iter_mut() {
        let elapsed = current_time.saturating_sub(motion.start_time);
        let mut progress = if motion.duration == 0 {
            1.0
        } else {
            elapsed as f32 / motion.duration as f32
        };

        if motion.repeat && progress >= 1.0 {
            progress %= 1.0;
        } else {
            progress = progress.clamp(0.0, 1.0);
        }

        if !motion.repeat && (elapsed >= motion.duration || motion.duration == 0) {
            finished.push((*target, property.clone()));
        }

        let eased = motion.easing.apply(progress);
        let value = motion.start_value.interpolate(&motion.end_value, eased);
        if state.values.get(&(*target, property.clone())) != Some(&value) {
            state.values.insert((*target, property.clone()), value);
            changed.push((*target, property.clone()));
        }
    }

    for key in finished {
        state.active.remove(&key);
        if state
            .presence
            .get(&key.0)
            .is_some_and(|phase| *phase == PresencePhase::Entering)
        {
            finished_presence.push((key.0, PresencePhase::Present));
        } else if state
            .presence
            .get(&key.0)
            .is_some_and(|phase| *phase == PresencePhase::Exiting)
        {
            finished_presence.push((key.0, PresencePhase::Hidden));
        }
    }

    for (id, phase) in finished_presence {
        state.presence.insert(id, phase);
    }

    changed
}

fn sync_tracks(
    state: &mut MotionStateMap,
    id: WidgetId,
    tracks: &[MotionTrack],
    runtime: &crate::RuntimeState,
    layout: Option<&LayoutSnapshot>,
    now: CurrentTime,
    requested: &mut HashSet<(WidgetId, MotionPropertyId)>,
    result: &mut MotionSyncResult,
) {
    let self_rect = layout.and_then(|layout| layout.get_node_rect(id));
    let input = MotionEvalInput {
        runtime,
        layout,
        self_id: id,
        self_rect,
        pointer_local: None,
    };
    for track in tracks {
        let key = (id, track.property.clone());
        requested.insert(key.clone());
        let target_value = track.to.eval(&input);
        if let Some(active) = state.active.get(&key) {
            if active.end_value == target_value
                && active.duration == track.transition.duration_ms()
                && active.repeat == track.transition.repeat_enabled()
                && active.frame_interval_ms == track.transition.frame_interval_value_ms()
                && active.easing == track.transition.easing()
            {
                continue;
            }
        }

        let current_value = state
            .values
            .get(&key)
            .cloned()
            .unwrap_or_else(|| track.property.default_value());
        if !track.transition.repeat_enabled()
            && state.values.contains_key(&key)
            && current_value == target_value
        {
            continue;
        }

        let start_value = match &track.from {
            MotionStartValue::Explicit(expr) => expr.eval(&input),
            MotionStartValue::Current => current_value,
        };

        state.values.insert(key.clone(), start_value.clone());
        state.active.insert(
            key.clone(),
            ActiveMotion {
                target: id,
                property: track.property.clone(),
                start_value,
                end_value: target_value,
                start_time: now + track.transition.delay_value_ms(),
                duration: track.transition.duration_ms(),
                repeat: track.transition.repeat_enabled(),
                frame_interval_ms: track.transition.frame_interval_value_ms(),
                easing: track.transition.easing(),
            },
        );
        result.changed.push(key);
    }
}

/// Creates a unitless scalar expression.
pub fn scalar(value: f32) -> MotionExpr {
    MotionExpr::Value(MotionValue::Scalar(value))
}

/// Creates a logical pixel expression.
pub fn px(value: f32) -> MotionExpr {
    MotionExpr::Value(MotionValue::Px(value))
}

/// Creates a degree expression.
pub fn deg(value: f32) -> MotionExpr {
    MotionExpr::Value(MotionValue::Deg(value))
}

/// Creates a color expression.
pub fn color(value: Color) -> MotionExpr {
    MotionExpr::Value(MotionValue::Color(value))
}

/// Creates an opacity fade-in track.
pub fn fade() -> Vec<MotionTrack> {
    vec![MotionTrack::composite(
        MotionPropertyId::Opacity,
        MotionStartValue::Explicit(scalar(0.0)),
        scalar(1.0),
    )]
}

/// Creates a horizontal slide-in track from `offset` pixels to zero.
pub fn slide_x(offset: f32) -> Vec<MotionTrack> {
    vec![MotionTrack::composite(
        MotionPropertyId::TranslateX,
        MotionStartValue::Explicit(px(offset)),
        px(0.0),
    )]
}

/// Creates a vertical slide-in track from `offset` pixels to zero.
pub fn slide_y(offset: f32) -> Vec<MotionTrack> {
    vec![MotionTrack::composite(
        MotionPropertyId::TranslateY,
        MotionStartValue::Explicit(px(offset)),
        px(0.0),
    )]
}

/// Creates a width collapse/expand track from zero to intrinsic width.
pub fn collapse_x() -> Vec<MotionTrack> {
    vec![MotionTrack {
        property: MotionPropertyId::Width,
        phase: MotionPhase::Layout,
        from: MotionStartValue::Explicit(px(0.0)),
        to: MotionExpr::IntrinsicWidth,
        transition: MotionTransition::default(),
    }]
}

/// Creates a height collapse/expand track from zero to intrinsic height.
pub fn collapse_y() -> Vec<MotionTrack> {
    vec![MotionTrack {
        property: MotionPropertyId::Height,
        phase: MotionPhase::Layout,
        from: MotionStartValue::Explicit(px(0.0)),
        to: MotionExpr::IntrinsicHeight,
        transition: MotionTransition::default(),
    }]
}

/// Creates tracks that follow another widget's x-position and width.
///
/// This is useful for tab indicators, segmented controls, and selected pills.
pub fn follow_x_and_width(target: WidgetId) -> Vec<MotionTrack> {
    vec![
        MotionTrack::composite(
            MotionPropertyId::TranslateX,
            MotionStartValue::Current,
            MotionExpr::LayoutX(target),
        ),
        MotionTrack {
            property: MotionPropertyId::Width,
            phase: MotionPhase::Layout,
            from: MotionStartValue::Current,
            to: MotionExpr::LayoutWidth(target),
            transition: MotionTransition::default(),
        },
    ]
}

/// Creates a hover/press scale feedback track for a widget id.
pub fn hover_press(id: WidgetId) -> Vec<MotionTrack> {
    vec![MotionTrack::composite(
        MotionPropertyId::Scale,
        MotionStartValue::Current,
        MotionExpr::If {
            predicate: MotionPredicate::Pressed(id),
            then_expr: Box::new(scalar(0.97)),
            else_expr: Box::new(MotionExpr::If {
                predicate: MotionPredicate::Hovered(id),
                then_expr: Box::new(scalar(1.02)),
                else_expr: Box::new(scalar(1.0)),
            }),
        },
    )
    .transition(MotionTransition::spring(420.0, 30.0))]
}

/// Returns the default ripple effect configuration.
pub fn ripple_effect() -> RippleFx {
    RippleFx::default()
}

/// Convenience wrapper for presence motion.
///
/// The provided `tracks` are used for enter motion and reversed for exit motion.
pub fn presence(
    id: impl IntoMotionId,
    visible: bool,
    tracks: Vec<MotionTrack>,
    child: impl Into<Widget>,
) -> Widget {
    Presence {
        id: id.into_motion_id(),
        visible,
        enter: tracks.clone(),
        exit: reverse_tracks_for_exit(&tracks),
        child: child.into(),
        ..Default::default()
    }
    .into()
}

/// Convenience wrapper for applying tracks to a child immediately.
pub fn appear(id: impl IntoMotionId, tracks: Vec<MotionTrack>, child: impl Into<Widget>) -> Widget {
    Motion {
        id: id.into_motion_id(),
        tracks,
        child: child.into(),
        ..Default::default()
    }
    .into()
}

/// Convenience alias for layout-oriented motion.
pub fn layout(id: impl IntoMotionId, tracks: Vec<MotionTrack>, child: impl Into<Widget>) -> Widget {
    appear(id, tracks, child)
}

/// Convenience alias for interaction-driven motion.
pub fn interactive(
    id: impl IntoMotionId,
    tracks: Vec<MotionTrack>,
    child: impl Into<Widget>,
) -> Widget {
    appear(id, tracks, child)
}

/// Convenience wrapper for adding a ripple layer to a child.
pub fn ripple(id: impl IntoMotionId, effect: RippleFx, child: impl Into<Widget>) -> Widget {
    RippleLayer {
        id: id.into_motion_id(),
        effect,
        child: child.into(),
    }
    .into()
}

/// Produces exit tracks by reversing explicit enter start values.
///
/// A track with `from: Explicit(x)` exits from the current value back to `x`.
/// A track with `from: Current` exits to the property's default value.
pub fn reverse_tracks_for_exit(tracks: &[MotionTrack]) -> Vec<MotionTrack> {
    tracks
        .iter()
        .map(|track| MotionTrack {
            property: track.property.clone(),
            phase: track.phase,
            from: MotionStartValue::Current,
            to: match &track.from {
                MotionStartValue::Explicit(expr) => expr.clone(),
                MotionStartValue::Current => track.property.default_value().into(),
            },
            transition: track.transition.clone(),
        })
        .collect()
}

/// Removes duplicate tracks so the last track for each property/phase wins.
///
/// Widget-owned motion presets use this to implement deterministic ordered
/// composition.
pub fn dedupe_tracks_later_wins(tracks: Vec<MotionTrack>) -> Vec<MotionTrack> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(tracks.len());
    for track in tracks.into_iter().rev() {
        if seen.insert((track.property.clone(), track.phase)) {
            out.push(track);
        }
    }
    out.reverse();
    out
}

impl From<MotionValue> for MotionExpr {
    fn from(value: MotionValue) -> Self {
        Self::Value(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Generic surface motion preset used by custom widgets and framework internals.
///
/// Built-in widgets expose widget-specific motion enums instead. Use
/// `SurfaceMotion` when authoring your own widget that needs a simple enter/exit
/// surface animation without defining a full public enum.
pub enum SurfaceMotion {
    /// Curated default: fade plus scale.
    Default,
    /// Fade opacity in.
    Fade,
    /// Scale from slightly smaller to normal size.
    Scale,
    /// Slide horizontally from the given pixel offset.
    SlideX(f32),
    /// Slide vertically from the given pixel offset.
    SlideY(f32),
    /// Compound fade plus scale preset.
    Pop,
    /// Ordered composition of surface motion atoms.
    Composition(Vec<SurfaceMotion>),
    /// Caller-provided native tracks.
    Custom {
        /// Enter tracks.
        enter: Vec<MotionTrack>,
        /// Exit tracks.
        exit: Vec<MotionTrack>,
        /// Whether the child stays rendered after exit completes.
        keep_rendered: bool,
    },
}

impl SurfaceMotion {
    /// Flattens and normalizes an ordered composition.
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

    /// Lowers this preset into enter tracks.
    pub fn enter_tracks(&self) -> Vec<MotionTrack> {
        let mut out = Vec::new();
        self.append_enter_tracks(&mut out);
        dedupe_tracks_later_wins(out)
    }

    /// Lowers this preset into exit tracks.
    pub fn exit_tracks(&self) -> Vec<MotionTrack> {
        match self {
            Self::Custom { exit, .. } => exit.clone(),
            _ => reverse_tracks_for_exit(&self.enter_tracks()),
        }
    }

    /// Returns whether this preset requests persistent rendering after exit.
    pub fn keep_rendered(&self) -> bool {
        match self {
            Self::Custom { keep_rendered, .. } => *keep_rendered,
            Self::Composition(items) => items.iter().any(Self::keep_rendered),
            _ => false,
        }
    }

    fn append_enter_tracks(&self, out: &mut Vec<MotionTrack>) {
        match self {
            Self::Default => {
                Self::Fade.append_enter_tracks(out);
                Self::Scale.append_enter_tracks(out);
            }
            Self::Fade => out.extend(fade()),
            Self::Scale => out.push(MotionTrack::composite(
                MotionPropertyId::Scale,
                MotionStartValue::Explicit(scalar(0.96)),
                scalar(1.0),
            )),
            Self::SlideX(offset) => out.extend(slide_x(*offset)),
            Self::SlideY(offset) => out.extend(slide_y(*offset)),
            Self::Pop => {
                Self::Fade.append_enter_tracks(out);
                Self::Scale.append_enter_tracks(out);
            }
            Self::Composition(items) => {
                for item in items {
                    item.append_enter_tracks(out);
                }
            }
            Self::Custom { enter, .. } => out.extend(enter.clone()),
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
}

impl Add for SurfaceMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

fn numeric_binary(
    a: &MotionExpr,
    b: &MotionExpr,
    input: &MotionEvalInput<'_>,
    f: impl FnOnce(f32, f32) -> f32,
) -> MotionValue {
    let left = a.eval(input);
    let right = b.eval(input).as_scalar_like().unwrap_or(0.0);
    map_numeric(left, |left| f(left, right))
}

fn numeric_unary(
    value: &MotionExpr,
    input: &MotionEvalInput<'_>,
    f: impl FnOnce(f32) -> f32,
) -> MotionValue {
    let value = value.eval(input);
    map_numeric(value, f)
}

fn map_numeric(value: MotionValue, f: impl FnOnce(f32) -> f32) -> MotionValue {
    match value {
        MotionValue::Scalar(v) => MotionValue::Scalar(f(v)),
        MotionValue::Px(v) => MotionValue::Px(f(v)),
        MotionValue::Deg(v) => MotionValue::Deg(f(v)),
        MotionValue::Bool(_) | MotionValue::Color(_) => value,
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
