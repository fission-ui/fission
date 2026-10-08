<!-- CLI {{CLI_VERSION}}; framework API {{FRAMEWORK_VERSION}}; guidance {{GUIDANCE_VERSION}}; schema {{SCHEMA_VERSION}} -->

# Browser widgets, state and routing

Use the target-neutral [shared application rules](../../../references/shared-app.md)
for widget boundaries, retained state, identity, reducers and asynchronous work.
The following guidance applies to browser routing.

Routing uses `Router` and `RouteParams` (not `RouterParams`). The shell seeds
`Env.current_route` and dispatches `ShellRouteChanged`; register a reducer with
the shell's `.with_route_handler(...)`. Store its logical path in app state:

```rust
use fission::prelude::*;
use fission::core::action::ShellRouteChanged;
#[derive(Debug, Clone, PartialEq)]
pub struct RouteState { pub path: String }
impl Default for RouteState { fn default() -> Self { Self { path: "/".into() } } }
impl GlobalState for RouteState {}
pub fn route_changed(state: &mut RouteState, action: ShellRouteChanged, _ctx: &mut ReducerContext<RouteState>) {
    state.path = action.location.pathname;
}
#[derive(Clone)]
pub struct RoutedApp;
#[derive(Clone)]
struct ItemPage;
impl From<ItemPage> for Widget {
    fn from(_: ItemPage) -> Widget {
        let params = fission::build::read::<RouteParams>();
        Text::new(params.get("id").cloned().unwrap_or_default()).into()
    }
}
impl From<RoutedApp> for Widget {
    fn from(_: RoutedApp) -> Widget {
        let (_, view) = fission::build::current::<RouteState>();
        Router::<RouteState>::new().with_path(view.state().path.clone())
            .route_component("/items/:id", ItemPage)
            .not_found(|| Text::new("Page not found")).into()
    }
}
```

Connect the existing shell action handler with
`WebApp::<RouteState, _>::new(RoutedApp).with_route_handler(route_changed)`;
do not generate a second action named `ShellRouteChanged`.

Navigate with `Link::to("Item 42", "/items/42")` or reducer effects (`ctx.effects.navigate(...)`,
`replace_route`, `navigation_back`, `navigation_forward`, `navigation_go`)
to keep browser history and shell routes consistent. Web Auto retains the
initial hash/path strategy; `WebApp::with_navigation(...)` can select Path/Hash.
Verify deep links and Back/Forward. Matching is ordered and exact by segment:
`/items/:id` does not match `/items/42/details`. Matched components read
`RouteParams` only in their conversion scope. Static protection cannot
authenticate a visitor.
