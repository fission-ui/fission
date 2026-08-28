use fission_core::build::{self, BuildCtxHandle, ViewHandle};
use fission_core::ui::Composite;
use fission_core::{GlobalState, Widget};
use fission_ir::WidgetId;
use std::collections::HashMap;
use std::sync::Arc;

/// Named path parameters captured while matching a route pattern.
///
/// A pattern such as `/projects/:project_id` inserts the matching path segment
/// under `project_id`. Query-string and fragment values are not included.
pub type RouteParams = HashMap<String, String>;

/// Type-erased builder invoked for the first matching route.
///
/// Builders receive the current build and view handles plus captured path
/// parameters. Prefer [`Router::route_component`] for ordinary retained
/// components; use this lower-level form when construction genuinely needs
/// route parameters or direct build context.
pub type PageBuilder<S> =
    Arc<dyn Fn(BuildCtxHandle<S>, ViewHandle<S>, &RouteParams) -> Widget + Send + Sync>;

/// One ordered route-table entry.
pub struct Route<S: GlobalState> {
    /// Exact-segment path pattern, optionally containing `:named` segments.
    pub path: String,
    /// Lazy component builder invoked only when `path` matches.
    pub builder: PageBuilder<S>,
}

/// Declarative, ordered router for a Fission application state type.
///
/// The router compares [`Router::current_path`] with each registered pattern
/// and constructs only the first matching route. This laziness is important:
/// inactive pages cannot register reducers, resources, jobs, or local state.
pub struct Router<S: GlobalState> {
    /// Current origin-free path used for matching.
    ///
    /// Applications normally mirror the shell route into `GlobalState` with a
    /// route handler and pass that value through [`Router::with_path`].
    pub current_path: String,
    /// Route entries in matching order; the first match wins.
    pub routes: Vec<Route<S>>,
    /// Optional fallback builder for paths not matched by `routes`.
    ///
    /// Without a custom fallback, the router renders a small `404` text node.
    pub not_found: Option<PageBuilder<S>>,
}

impl<S: GlobalState> From<Router<S>> for Widget {
    fn from(component: Router<S>) -> Self {
        let (ctx, view) = build::current::<S>();
        let this = &component;

        for route in &this.routes {
            if let Some(params) = match_route(&route.path, &this.current_path) {
                return build::provide(params.clone(), || {
                    route_scoped_widget(
                        &format!("{}=>{}", route.path, this.current_path),
                        (route.builder)(ctx, view, &params),
                    )
                });
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
    /// Creates an empty router initially matching `/`.
    pub fn new() -> Self {
        Self {
            current_path: "/".to_string(),
            routes: Vec::new(),
            not_found: None,
        }
    }

    /// Sets the origin-free path to match during this build.
    ///
    /// The value may contain the shell-provided query and fragment, but route
    /// pattern matching currently operates on path segments only.
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.current_path = path.into();
        self
    }

    /// Registers a lazy zero-argument route builder.
    ///
    /// Use this for simple pages that need neither route parameters nor direct
    /// build handles. Routes are tested in registration order.
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

    /// Registers a retained component and converts it only when its route matches.
    ///
    /// This is the preferred API for component values such as
    /// `ProtectedRoute::new(decision, AccountPage)`. The component is cloned as
    /// a value for the route table, but inactive branches are not converted to
    /// [`Widget`] values. A matching component can read captured parameters
    /// with `fission::build::read::<RouteParams>()`.
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

    /// Registers a fully typed [`PageBuilder`].
    ///
    /// Use this when the page needs captured [`RouteParams`] or direct access
    /// to its build/view handles.
    pub fn route_builder(mut self, path: impl Into<String>, builder: PageBuilder<S>) -> Self {
        self.routes.push(Route {
            path: path.into(),
            builder,
        });
        self
    }

    /// Installs a lazy fallback for paths that do not match any route.
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
    use std::sync::Mutex;

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

    #[derive(Clone)]
    struct ParameterPage(Arc<Mutex<Option<String>>>);

    impl From<ParameterPage> for Widget {
        fn from(page: ParameterPage) -> Self {
            let params = build::read::<RouteParams>();
            let project_id = params.get("project_id").cloned().unwrap_or_default();
            *page.0.lock().unwrap() = Some(project_id.clone());
            Text::new(project_id).into()
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

    #[test]
    fn route_component_provides_captured_parameters_during_conversion() {
        let observed = Arc::new(Mutex::new(None));
        let router = Router::<State>::new()
            .with_path("/projects/42")
            .route_component("/projects/:project_id", ParameterPage(observed.clone()));
        let state = State;
        let runtime = RuntimeState::default();
        let env = Env::default();
        let view = View::new(&state, &runtime, &env, None);
        let mut ctx = BuildCtx::<State>::new();

        let _ = fission_core::build::enter(&mut ctx, &view, || -> Widget { router.into() });

        assert_eq!(observed.lock().unwrap().as_deref(), Some("42"));
    }
}
