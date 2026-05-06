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
/// fn build(&self, _ctx: &mut BuildCtx<MyState>, view: &View<MyState>) -> Node {
///     let name = &view.state.user_name;
///     let theme = view.theme();
///     Text::new(format!("Hello, {}!", name))
///         .color(theme.tokens.colors.primary)
///         .into_node()
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

/// The core trait for composable UI components.
///
/// A `Widget` produces a [`Node`] tree given read-only access to state
/// ([`View`]) and a mutable build context ([`BuildCtx`]) for binding actions,
/// registering portals, and requesting animations.
///
/// # Example
///
/// ```rust,ignore
/// struct Greeting;
///
/// impl Widget<AppState> for Greeting {
///     fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> Node {
///         let on_press = ctx.bind(SayHello, handle_hello as fn(&mut AppState, SayHello));
///         Button {
///             child: Some(Box::new(Text::new("Hello!").into_node())),
///             on_press: Some(on_press),
///             ..Default::default()
///         }.into_node()
///     }
/// }
/// ```
pub trait Widget<S: AppState> {
    /// Build the widget's node tree.
    ///
    /// Called once per frame. Implementations must be pure -- all side-effects
    /// go through `ctx` (action binding, portals, animations).
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node;
}

// Implement Widget for Node (identity)
impl<S: AppState> Widget<S> for Node {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        self.clone()
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
