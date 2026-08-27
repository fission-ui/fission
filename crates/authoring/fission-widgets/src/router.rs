use fission_core::build::{self, BuildCtxHandle, ViewHandle};
use fission_core::ui::Composite;
use fission_core::{GlobalState, Widget};
use fission_ir::WidgetId;
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
                return route_scoped_widget(
                    &format!("{}=>{}", route.path, this.current_path),
                    (route.builder)(ctx, view, &params),
                );
            }
        }

        if let Some(not_found) = &this.not_found {
            return route_scoped_widget(
                &format!("not_found=>{}", this.current_path),
                not_found(ctx, view, &HashMap::new()),
            );
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

    /// Registers a concrete component and converts it only when its route matches.
    pub fn route_component<W>(mut self, path: impl Into<String>, component: W) -> Self
    where
        W: Clone + Into<Widget> + Send + Sync + 'static,
    {
        self.routes.push(Route {
            path: path.into(),
            builder: Arc::new(move |_ctx, _view, _| component.clone().into()),
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

fn route_scoped_widget(route_key: &str, child: Widget) -> Widget {
    Composite {
        id: Some(WidgetId::explicit(&format!(
            "fission.router.route:{route_key}"
        ))),
        child,
        ..Default::default()
    }
    .into()
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

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::internal::BuildCtx;
    use fission_core::{Env, RuntimeState, Text, View};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Default)]
    struct State;
    impl GlobalState for State {}

    #[derive(Clone)]
    struct CountedPage(Arc<AtomicUsize>);

    impl From<CountedPage> for Widget {
        fn from(page: CountedPage) -> Self {
            page.0.fetch_add(1, Ordering::SeqCst);
            Text::new("matched").into()
        }
    }

    #[test]
    fn route_component_converts_only_the_matching_component() {
        let unmatched = Arc::new(AtomicUsize::new(0));
        let matched = Arc::new(AtomicUsize::new(0));
        let router = Router::<State>::new()
            .with_path("/account")
            .route_component("/settings", CountedPage(unmatched.clone()))
            .route_component("/account", CountedPage(matched.clone()));
        let state = State;
        let runtime = RuntimeState::default();
        let env = Env::default();
        let view = View::new(&state, &runtime, &env, None);
        let mut ctx = BuildCtx::<State>::new();

        let _ = fission_core::build::enter(&mut ctx, &view, || -> Widget { router.into() });

        assert_eq!(unmatched.load(Ordering::SeqCst), 0);
        assert_eq!(matched.load(Ordering::SeqCst), 1);
    }
}
