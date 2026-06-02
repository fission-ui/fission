use crate::build::{self, BuildCtxHandle, ViewHandle};
use crate::ui::{Container, Widget};
use crate::{BoxConstraints, GlobalState, WidgetId};
use std::sync::Arc;

/// A closure-backed widget for small inline composition.
///
/// `Builder` is not a replacement for named components. Use
/// `impl From<MyComponent> for Widget` for reusable application widgets. Use
/// `Builder` when a short local closure is clearer than introducing a named
/// component, for example inside tests or small conditional sections.
#[derive(Clone)]
pub struct Builder<S: GlobalState> {
    builder: Arc<dyn Fn(BuildCtxHandle<S>, ViewHandle<S>) -> Widget + Send + Sync>,
}

impl<S: GlobalState> Builder<S> {
    pub fn new<F>(builder: F) -> Self
    where
        F: Fn(BuildCtxHandle<S>, ViewHandle<S>) -> Widget + Send + Sync + 'static,
    {
        Self {
            builder: Arc::new(builder),
        }
    }
}

impl<S: GlobalState> From<Builder<S>> for Widget {
    fn from(component: Builder<S>) -> Self {
        let (ctx, view) = build::current::<S>();
        (component.builder)(ctx, view)
    }
}

/// A closure-backed widget that receives its parent's last known constraints.
///
/// The constraint value comes from the previous layout pass when an explicit
/// `id` is supplied. Otherwise it falls back to a loose constraint derived from
/// the current viewport. Prefer normal responsive widgets first; use
/// `LayoutBuilder` when the child tree itself must change with available size.
#[derive(Clone)]
pub struct LayoutBuilder<S: GlobalState> {
    /// Optional stable identity for constraint look-up across frames.
    pub id: Option<WidgetId>,
    /// Flex grow factor applied to the wrapper when `id` is set.
    pub flex_grow: f32,
    /// Flex shrink factor applied to the wrapper when `id` is set.
    pub flex_shrink: f32,
    builder: Arc<dyn Fn(BuildCtxHandle<S>, ViewHandle<S>, BoxConstraints) -> Widget + Send + Sync>,
}

impl<S: GlobalState> LayoutBuilder<S> {
    pub fn new<F>(builder: F) -> Self
    where
        F: Fn(BuildCtxHandle<S>, ViewHandle<S>, BoxConstraints) -> Widget + Send + Sync + 'static,
    {
        Self {
            id: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            builder: Arc::new(builder),
        }
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow;
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = shrink;
        self
    }
}

impl<S: GlobalState> From<LayoutBuilder<S>> for Widget {
    fn from(component: LayoutBuilder<S>) -> Self {
        let (ctx, view) = build::current::<S>();
        let viewport = view.viewport_size();
        let mut max_w = viewport.width;
        let mut max_h = viewport.height;
        if !max_w.is_finite() || max_w <= 0.0 {
            max_w = f32::INFINITY;
        }
        if !max_h.is_finite() || max_h <= 0.0 {
            max_h = f32::INFINITY;
        }
        let fallback = BoxConstraints::loose(max_w, max_h);
        let id = build::current_widget_id().or(component.id);
        let constraints = id
            .and_then(|id| view.get_constraints(id))
            .unwrap_or(fallback);
        let child = (component.builder)(ctx, view, constraints);

        if let Some(id) = id {
            let mut container = Container::new(child)
                .flex_grow(component.flex_grow)
                .flex_shrink(component.flex_shrink);
            container.id = Some(id);
            container.into()
        } else {
            child
        }
    }
}
