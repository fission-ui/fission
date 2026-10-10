<!-- CLI {{CLI_VERSION}}; framework API {{FRAMEWORK_VERSION}}; guidance {{GUIDANCE_VERSION}}; schema {{SCHEMA_VERSION}} -->

# Shared application rules for every target

Trace the actual app entrypoint, screen and component before editing. Keep shared
state, reducers, routes, widgets, effects/services, design and i18n in the app
core; target shell entrypoints configure and run it. Use supported scaffolding
commands instead of duplicating platform boilerplate. Compile the changed APIs
and exercise the requested target.

Model meaningful UI as concrete structs implementing `From<YourWidget> for Widget`.
Prefer a reusable widget per file; separate state, routes, screens,
widgets, effects and design modules. Keep Rust files below 2,000 lines.
Small formatting/leaf helpers are fine; avoid hiding reusable trees in anonymous
screen functions.

`GlobalState` holds durable/shared app truth. Reducers update it; conversion reads
it. Match dispatched actions to their registered reducers. Use
`#[fission_component]`/`#[local_state]` for retained memory owned by one widget
identity, and `ctx.bind_local(...)` for its transitions. `Env` carries presentation
inputs; providers carry build-scope context. Never retain `BuildCtxHandle` or
`ViewHandle` in structures, reducers, services, async tasks or statics.
Use `fission::build::current::<AppState>()` for concrete app state;
`::<()>` is only for intentionally state-agnostic components.

Never mutate global state, perform I/O or launch host work during conversion.
Request asynchronous work through effects/jobs/services/capabilities/resources;
retain loading/error state and a useful loader. Never block the UI thread.
Production paths use real data, explicit empty states or errors; fixtures belong
in tests.

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
`.id(WidgetId::explicit(...))`, not list indices. Give interactive controls and
semantic regions stable identifiers for accessibility/testing.

Use Fission's `Router` and `RouteParams` for routed screens. Read route parameters
only within the matched component's conversion scope; intentionally align
frontend, deep-link and backend route shapes. Browser navigation and browser
testing instructions are conditional on a Web task, not requirements for a
native or Terminal app.
