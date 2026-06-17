use fission_core::build::{self, BuildCtxHandle, ViewHandle};
use fission_core::{GlobalState, Widget};
use std::collections::HashMap;
use std::sync::Arc;

pub type RouteParams = HashMap<String, String>;
pub type PageBuilder<S> =
    Arc<dyn Fn(BuildCtxHandle<S>, ViewHandle<S>, &RouteParams) -> Widget + Send + Sync>;

pub struct Route<S: GlobalState> {
    pub path: String,
    pub builder: PageBuilder<S>,
}

pub struct Router<S: GlobalState> {
    pub current_path: String,
    pub routes: Vec<Route<S>>,
    pub not_found: Option<PageBuilder<S>>,
}

impl<S: GlobalState> From<Router<S>> for Widget {
    fn from(component: Router<S>) -> Self {
        let (ctx, view) = build::current::<S>();
        let this = &component;

        for route in &this.routes {
            if let Some(params) = match_route(&route.path, &this.current_path) {
                return (route.builder)(ctx, view, &params);
            }
        }

        if let Some(not_found) = &this.not_found {
            return not_found(ctx, view, &HashMap::new());
        }

        fission_core::ui::Text::new(format!("404: {}", this.current_path)).into()
    }
}

impl<S: GlobalState> Router<S> {
    pub fn new() -> Self {
        Self {
            current_path: "/".to_string(),
            routes: Vec::new(),
            not_found: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.current_path = path.into();
        self
    }

    pub fn route<W, F>(mut self, path: impl Into<String>, builder: F) -> Self
    where
        W: Into<Widget>,
        F: Fn() -> W + Send + Sync + 'static,
    {
        self.routes.push(Route {
            path: path.into(),
            builder: Arc::new(move |_ctx, _view, _| builder().into()),
        });
        self
    }

    pub fn route_builder(mut self, path: impl Into<String>, builder: PageBuilder<S>) -> Self {
        self.routes.push(Route {
            path: path.into(),
            builder,
        });
        self
    }

    pub fn not_found<W, F>(mut self, builder: F) -> Self
    where
        W: Into<Widget>,
        F: Fn() -> W + Send + Sync + 'static,
    {
        self.not_found = Some(Arc::new(move |_ctx, _view, _| builder().into()));
        self
    }
}

// Simple route matcher: "/users/:id" matches "/users/123" -> {"id": "123"}
fn match_route(pattern: &str, path: &str) -> Option<RouteParams> {
    let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if pattern_parts.len() != path_parts.len() {
        return None;
    }

    let mut params = HashMap::new();
    for (pat, segment) in pattern_parts.iter().zip(path_parts.iter()) {
        if pat.starts_with(':') {
            params.insert(pat[1..].to_string(), segment.to_string());
        } else if pat != segment {
            return None;
        }
    }

    Some(params)
}
