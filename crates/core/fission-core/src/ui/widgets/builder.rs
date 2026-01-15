use crate::{AppState, BuildCtx, BoxConstraints, Node, NodeId, View, Widget, WidgetNodeId};
use crate::ui::Container;
use std::sync::Arc;

pub struct Builder<S: AppState> {
    builder: Arc<dyn Fn(&mut BuildCtx<S>, &View<S>) -> Node + Send + Sync>,
}

impl<S: AppState> Builder<S> {
    pub fn new<F>(builder: F) -> Self
    where
        F: Fn(&mut BuildCtx<S>, &View<S>) -> Node + Send + Sync + 'static,
    {
        Self {
            builder: Arc::new(builder),
        }
    }
}

impl<S: AppState> Widget<S> for Builder<S> {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        (self.builder)(ctx, view)
    }
}

pub struct LayoutBuilder<S: AppState> {
    pub id: Option<WidgetNodeId>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    builder: Arc<dyn Fn(&mut BuildCtx<S>, &View<S>, BoxConstraints) -> Node + Send + Sync>,
}

impl<S: AppState> LayoutBuilder<S> {
    pub fn new<F>(builder: F) -> Self
    where
        F: Fn(&mut BuildCtx<S>, &View<S>, BoxConstraints) -> Node + Send + Sync + 'static,
    {
        Self {
            id: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            builder: Arc::new(builder),
        }
    }

    pub fn id(mut self, id: WidgetNodeId) -> Self {
        self.id = Some(id);
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
}

impl<S: AppState> Widget<S> for LayoutBuilder<S> {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let viewport = view
            .layout
            .map(|layout| layout.viewport_size)
            .unwrap_or_else(|| view.viewport_size());
        let mut max_w = viewport.width;
        let mut max_h = viewport.height;
        if !max_w.is_finite() || max_w <= 0.0 {
            max_w = f32::INFINITY;
        }
        if !max_h.is_finite() || max_h <= 0.0 {
            max_h = f32::INFINITY;
        }
        let fallback = BoxConstraints::loose(max_w, max_h);
        let constraints = self
            .id
            .and_then(|id| view.get_constraints(id))
            .unwrap_or(fallback);
        let child = (self.builder)(ctx, view, constraints);
        if let Some(id) = self.id {
            Container::new(child)
                .id(NodeId::from(id))
                .flex_grow(self.flex_grow)
                .flex_shrink(self.flex_shrink)
                .into_node()
        } else {
            child
        }
    }
}
