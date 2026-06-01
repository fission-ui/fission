//! Read-only view, widget trait, and selector pattern.
//!
//! During [`Widget::build`], the framework provides a [`View`] that gives
//! read-only access to the current [`AppState`], theme, i18n registry,
//! layout snapshot, and animation values. Widgets use this to decide what
//! to render without any side-effects.

use crate::{
    env::VideoState,
    registry::{AnimationPropertyId, VideoRegistration},
    ui::{
        Align, Button, Checkbox, Column, Container, Grid, GridItem, Image, LazyColumn, Node,
        Overlay, Positioned, Radio, Row, Scroll, Slider, Spacer, Switch, Text, TextInput, Video,
        ZStack,
    },
    AppState, BuildCtx, Env, LayoutRect, LayoutSize, LayoutSnapshot, RuntimeState,
};
use fission_i18n::I18nRegistry;
use fission_ir::{NodeId, WidgetNodeId};
use fission_layout::BoxConstraints;
use fission_theme::Theme;
use std::sync::Arc;

/// Read-only access to application state and environment during widget building.
///
/// `View` is the primary way widgets read data. It is parameterised over the
/// concrete [`AppState`] type `S`, giving type-safe access to `state` while
/// also exposing the theme, i18n registry, layout snapshot from the previous
/// frame, and animation values.
///
/// # Example
///
/// ```rust,ignore
/// fn build(
///     &self,
///     _ctx: &mut BuildCtx<MyState>,
///     view: &View<MyState>,
/// ) -> impl IntoWidget<MyState> {
///     let name = &view.state.user_name;
///     let theme = view.theme();
///     Text::new(format!("Hello, {}!", name))
///         .color(theme.tokens.colors.primary)
/// }
/// ```
pub struct View<'a, S: AppState> {
    /// Reference to the current application state.
    pub state: &'a S,
    /// Runtime interaction, scroll, text-edit, and animation state.
    pub runtime: &'a RuntimeState,
    /// Environment (theme, i18n, viewport size, locale).
    pub env: &'a Env,
    /// Layout snapshot from the previous frame, if available.
    pub layout: Option<&'a LayoutSnapshot>,
}

impl<'a, S: AppState> View<'a, S> {
    pub fn new(
        state: &'a S,
        runtime: &'a RuntimeState,
        env: &'a Env,
        layout: Option<&'a LayoutSnapshot>,
    ) -> Self {
        Self {
            state,
            runtime,
            env,
            layout,
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.env.theme
    }
    pub fn i18n(&self) -> &I18nRegistry {
        &self.env.i18n
    }

    pub fn get_rect(&self, id: WidgetNodeId) -> Option<LayoutRect> {
        let node_id: NodeId = id.into();
        self.layout.and_then(|l| l.get_node_rect(node_id))
    }

    pub fn get_constraints(&self, id: WidgetNodeId) -> Option<BoxConstraints> {
        let node_id: NodeId = id.into();
        self.layout.and_then(|l| l.get_node_constraints(node_id))
    }

    pub fn viewport_size(&self) -> LayoutSize {
        self.env.viewport_size
    }

    pub fn select<T: Selector<S>>(&self) -> T::Output {
        T::select(self)
    }

    pub fn animation_value(&self, widget_id: WidgetNodeId, property: &AnimationPropertyId) -> f32 {
        self.runtime
            .animation
            .values
            .get(&(widget_id, property.clone()))
            .copied()
            .unwrap_or_else(|| property.default_value())
    }

    pub fn video_state(&self, widget_id: WidgetNodeId) -> Option<&VideoState> {
        self.runtime.video.states.get(&widget_id)
    }
}

/// A selector that derives a value from the [`View`].
///
/// Selectors extract and transform data from state so widgets can depend on
/// derived values without coupling to the full state shape.
///
/// # Example
///
/// ```rust,ignore
/// struct ItemCount;
/// impl Selector<MyState> for ItemCount {
///     type Output = usize;
///     fn select(view: &View<MyState>) -> usize {
///         view.state.items.len()
///     }
/// }
///
/// // In a widget:
/// let count: usize = view.select::<ItemCount>();
/// ```
pub trait Selector<S: AppState> {
    /// The type produced by the selector.
    type Output;
    /// Extract the value from the given view.
    fn select(view: &View<S>) -> Self::Output;
}

/// Type-erased storage for a Fission widget.
///
/// Application authors should rarely need to name this type directly. It exists
/// because widget composition needs to store heterogeneous child widgets in
/// collections such as columns, rows, routes, overlays, and slots while still
/// allowing each widget's `build` method to return a concrete Rust type.
///
/// A tempting alternative is `Vec<Box<dyn Widget<S>>>`, but that would force
/// [`Widget`] to be object-safe. The normal Fission build signature intentionally
/// is not object-safe: it returns `impl IntoWidget<S>` so component authors can
/// return ordinary structs without boxing, dynamic dispatch, or manually naming
/// an erased type. Rust cannot put a trait method returning `impl Trait` into a
/// trait-object vtable, so `Box<dyn Widget<S>>` is not the right public model.
///
/// `AnyWidget<S>` is the narrow internal erasure point that solves that problem:
/// public APIs accept `impl IntoWidget<S>`, user components implement
/// [`Widget`], and Fission stores mixed children as `AnyWidget<S>` only after
/// the value has entered the framework.
pub struct AnyWidget<S: AppState> {
    inner: AnyWidgetInner<S>,
}

enum AnyWidgetInner<S: AppState> {
    Node(Node),
    Widget(Arc<dyn ErasedWidget<S>>),
}

impl<S: AppState> Clone for AnyWidget<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S: AppState> Clone for AnyWidgetInner<S> {
    fn clone(&self) -> Self {
        match self {
            Self::Node(node) => Self::Node(node.clone()),
            Self::Widget(widget) => Self::Widget(Arc::clone(widget)),
        }
    }
}

trait ErasedWidget<S: AppState>: Send + Sync {
    fn build_node(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node;
}

impl<S, W> ErasedWidget<S> for W
where
    S: AppState,
    W: Widget<S> + Send + Sync + 'static,
{
    fn build_node(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        Widget::build_node(self, ctx, view)
    }
}

impl<S: AppState> AnyWidget<S> {
    /// Create an erased widget from a normal Fission widget.
    pub fn new<W>(widget: W) -> Self
    where
        W: Widget<S> + Send + Sync + 'static,
    {
        Self {
            inner: AnyWidgetInner::Widget(Arc::new(widget)),
        }
    }

    /// Create an erased widget from a pre-lowered node.
    ///
    /// This is retained for framework internals and the migration of existing
    /// built-in widgets. New application code should return concrete widgets
    /// from `build`, not manually construct or return `Node`.
    #[doc(hidden)]
    pub fn from_node(node: Node) -> Self {
        Self {
            inner: AnyWidgetInner::Node(node),
        }
    }

    /// Build this erased widget to the internal node representation.
    #[doc(hidden)]
    pub fn build_node(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        match &self.inner {
            AnyWidgetInner::Node(node) => node.clone(),
            AnyWidgetInner::Widget(widget) => widget.build_node(ctx, view),
        }
    }
}

/// Converts a value into Fission's erased widget storage.
///
/// This is Fission-specific rather than `Into<Widget>` for two reasons:
///
/// - `Widget` is a trait, not a concrete storage type, so `Into<Widget>` is not
///   a valid Rust target. `Into<Box<dyn Widget<S>>>` would require an
///   object-safe [`Widget`] trait, which conflicts with the ergonomic
///   `build -> impl IntoWidget<S>` model.
/// - Fission owns this trait, so the framework can preserve clear diagnostics
///   and attach future metadata such as stable identity, source labels, hot
///   reload hints, and devtools information without exposing the internal node
///   representation as the authoring API.
///
/// Normal application code does not need to call this trait directly. Use
/// `impl Widget<S>` structs and pass concrete widget values to child slots.
pub trait IntoWidget<S: AppState>: Send + Sync + 'static {
    /// Convert this value into Fission's erased widget storage.
    fn into_widget(self) -> AnyWidget<S>;
}

impl<S, W> IntoWidget<S> for W
where
    S: AppState,
    W: Widget<S> + Send + Sync + 'static,
{
    fn into_widget(self) -> AnyWidget<S> {
        AnyWidget::new(self)
    }
}

/// The core trait for composable UI components.
///
/// A `Widget` produces another widget-like value given read-only access to
/// state ([`View`]) and a mutable build context ([`BuildCtx`]) for binding
/// actions, registering portals, and requesting animations. The returned value
/// is converted into Fission's private node tree by [`IntoWidget`].
///
/// # Example
///
/// ```rust,ignore
/// struct Greeting;
///
/// impl Widget<AppState> for Greeting {
///     fn build(
///         &self,
///         ctx: &mut BuildCtx<AppState>,
///         view: &View<AppState>,
///     ) -> impl IntoWidget<AppState> {
///         Text::new(format!("Hello, {}", view.state.name))
///     }
/// }
/// ```
pub trait Widget<S: AppState> {
    /// Build this widget.
    ///
    /// Called once per frame. Implementations must be pure -- all side-effects
    /// go through `ctx` (action binding, portals, animations).
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl IntoWidget<S>;

    /// Build this widget directly into Fission's internal node representation.
    ///
    /// Framework internals use this when lowering the root widget or composing
    /// legacy node-backed widgets. Application code should call `build` and
    /// return widgets, not nodes.
    #[doc(hidden)]
    fn build_node(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        self.build(ctx, view).into_widget().build_node(ctx, view)
    }
}

impl<S: AppState> Widget<S> for Node {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        self.clone()
    }

    fn build_node(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        self.clone()
    }
}

impl<S: AppState> Widget<S> for AnyWidget<S> {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> AnyWidget<S> {
        self.clone()
    }

    fn build_node(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        AnyWidget::build_node(self, ctx, view)
    }
}

macro_rules! impl_widget_for_primitive {
    ($t:ty, $v:ident) => {
        impl<S: AppState> Widget<S> for $t {
            fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
                Node::$v(self.clone())
            }
        }
    };
}

impl_widget_for_primitive!(Row, Row);
impl_widget_for_primitive!(Column, Column);
impl_widget_for_primitive!(Align, Align);
impl_widget_for_primitive!(Text, Text);
impl_widget_for_primitive!(Button, Button);
impl_widget_for_primitive!(TextInput, TextInput);
impl_widget_for_primitive!(Scroll, Scroll);
impl_widget_for_primitive!(Image, Image);
impl_widget_for_primitive!(ZStack, ZStack);
impl_widget_for_primitive!(Overlay, Overlay);
impl_widget_for_primitive!(Container, Container);
impl_widget_for_primitive!(Grid, Grid);
impl_widget_for_primitive!(GridItem, GridItem);
impl_widget_for_primitive!(Checkbox, Checkbox);
impl_widget_for_primitive!(Switch, Switch);
impl_widget_for_primitive!(Radio, Radio);
impl_widget_for_primitive!(Positioned, Positioned);
impl_widget_for_primitive!(Spacer, Spacer);
impl_widget_for_primitive!(Slider, Slider);
impl_widget_for_primitive!(LazyColumn, LazyColumn);

impl<S: AppState> Widget<S> for Video {
    fn build(&self, ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        let mut video = self.clone();
        let id = video
            .id
            .unwrap_or_else(|| WidgetNodeId::explicit(&video.source));
        video.id = Some(id);

        ctx.register_video(VideoRegistration {
            node_id: id,
            source: video.source.clone(),
            autoplay: video.autoplay,
            loop_playback: video.loop_playback,
        });

        Node::Video(video)
    }
}
