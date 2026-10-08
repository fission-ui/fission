<!-- CLI {{CLI_VERSION}}; framework API {{FRAMEWORK_VERSION}}; guidance {{GUIDANCE_VERSION}}; schema {{SCHEMA_VERSION}} -->

# Widgets, state and routing

Model meaningful UI as concrete structs implementing `From<YourWidget> for
Widget`. Prefer a reusable widget per file; separate state, routes, screens,
widgets, effects and design modules. Keep Rust files below 2,000 lines.
Small formatting/leaf helpers are fine; avoid hiding reusable trees in anonymous
screen functions.

`GlobalState` holds shared app truth. Reducers update it; conversion reads it.
`#[fission_component]`/`#[local_state]` retain widget memory. `Env` carries
presentation inputs; providers carry build-scope context. Never retain
`BuildCtxHandle`/`ViewHandle` in long-lived structures or jobs.
Use `fission::build::current::<AppState>()` for the concrete app state;
`::<()>` is only for state-agnostic components.

Never mutate global state, perform I/O or launch host work during conversion.
Request asynchronous work through effects/jobs/services/capabilities/resources;
retain loading/error state and a useful loader. Never block the UI thread.
Production paths use real data, empty states or errors; fixtures belong in tests.

```rust
use fission::prelude::*;
#[fission_component]
pub struct Disclosure {
    pub title: String,
    #[local_state(default = false)]
    open: bool,
}
#[fission_reducer(ToggleOpen)]
fn on_toggle_open(open: &mut bool) { *open = !*open; }
impl From<Disclosure> for Widget {
    fn from(section: Disclosure) -> Widget {
        let (ctx, _) = fission::build::current::<()>();
        let open = section.open();
        let toggle = ctx.bind_local(ToggleOpen, open.clone(), reduce!(on_toggle_open));
        Column {
            children: widgets![
                Button { on_press: Some(toggle), child: Some(Text::new(section.title).into()), ..Default::default() }
                    .semantics_identifier("disclosure.toggle"),
                Text::new(if open.get() { "Details visible" } else { "Details hidden" }),
            ],
            ..Default::default()
        }.into()
    }
}
```

For reorderable/filterable local components, assign durable identities with
`.id(WidgetId::explicit(...))`, not list indices. Give controls and semantic
regions stable identifiers for accessibility/testing.

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
