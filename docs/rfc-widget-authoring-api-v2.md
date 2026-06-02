# RFC: Widget Authoring API v2

Status: Draft  
Target release: 0.4.0  
Scope: Authoring API, widget tree construction, scoped build context, local widget state, global app state

---

## 1. Summary

Fission's public authoring API will be rebuilt around a single public tree type named `Widget`.
The current public `Widget<S>` trait and the public `Node` tree carrier will be removed.
The current `Node` concept becomes the public closed `Widget` enum.
Custom application widgets are normal Rust structs that convert into `Widget` with the standard `From<T> for Widget` trait.

The primary authoring pattern becomes:

```rust
struct MyPanel {
    title: String,
}

impl From<MyPanel> for Widget {
    fn from(panel: MyPanel) -> Widget {
        Card::new(
            Column {
                children: widgets![
                    Text::headline(panel.title),
                    Text::body("Hello from Fission"),
                ],
                ..Default::default()
            },
        )
        .into()
    }
}
```

The component macro is the recommended API for local retained widget state:

```rust
#[fission_component]
struct Counter {
    title: String,

    #[local_state(default = 0)]
    count: i32,
}

#[fission_reducer(Decrement)]
fn decrement_count(count: &mut i32) {
    *count -= 1;
}

#[fission_reducer(Increment)]
fn increment_count(count: &mut i32) {
    *count += 1;
}

impl From<Counter> for Widget {
    fn from(counter: Counter) -> Widget {
        let (ctx, _) = fission::build::current::<()>();
        let count = counter.count();
        let decrement = ctx.bind_local(Decrement, count, reduce!(decrement_count));
        let increment = ctx.bind_local(Increment, count, reduce!(increment_count));

        Center::new(
            Column {
                children: widgets![
                    Text::headline(counter.title),
                    Text::body(format!("Count: {}", count.get())),
                    Row {
                        children: widgets![
                            Button::new("Decrement", decrement),
                            Button::primary("Increment", increment),
                        ],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        )
        .into()
    }
}
```

This RFC intentionally does not preserve backwards compatibility.
The following public patterns are removed:

- `Node` as a public authoring type;
- `Widget<S>` as a public trait;
- `AnyWidget`;
- `IntoWidget`;
- `build_node` or any equivalent public or semi-public escape hatch;
- `into_node` as a user-facing method;
- builder-style repeated child append APIs such as `.child(...).child(...)`;
- public APIs that require every built-in widget to be generic over application state.

The resulting model has one visible tree type, one visible composition pattern, and explicit terminology for state:

- `Widget`: the closed public widget tree value;
- custom widgets/components: normal Rust structs converted into `Widget`;
- `GlobalState`: app-wide domain state owned by the app instance;
- `LocalWidgetState`: retained UI state owned by one widget identity;
- `BuildCtxHandle<S>` and `ViewHandle<S>`: scoped handles used during widget conversion.

---

## 2. Motivation

The existing authoring API has accumulated conflicting ideas:

1. A public `Widget<S>` trait suggests that user widgets are long-lived objects whose `build` method is the authoring boundary.
2. A public `Node` enum is still the actual normalized tree passed to lowering, layout, rendering, testing, and shells.
3. Many built-in widget types are generic over the app state type `S`, even though most widgets never inspect or mutate app state.
4. Attempts to hide `Node` behind `AnyWidget`, `IntoWidget`, `build_node`, or private lowering helpers still leaked `Node` through side doors.
5. Builder-style child appending made complex widget trees harder to read and pushed examples toward APIs that do not resemble the intended declarative tree shape.

The API should make the correct architecture the easiest thing to write.
It must be clear enough that documentation, examples, and AI-generated code naturally follow the same pattern.

The target developer experience is:

- write plain Rust structs for custom UI components;
- convert those structs into `Widget` with `impl From<T> for Widget`;
- compose trees directly with public struct fields, constructors where useful, and `widgets![...]`;
- use `#[local_state]` only for retained widget-local state;
- use `GlobalState` explicitly for app/domain state;
- use scoped build handles only when a component needs global state, actions, services, environment, or platform context.

---

## 3. Problems With The Current API

### 3.1 `Node` leaks because it is the real tree

The current model treats `Node` as an internal tree carrier, but many public APIs need to store children.
Those children must have one concrete type.
If that concrete type is `Node`, it appears everywhere:

```rust
pub struct Column {
    pub children: Vec<Node>,
}
```

If the public API then tries to hide `Node`, every child slot needs an alternative carrier.
Previous attempts created `AnyWidget`, `IntoWidget`, `build_node`, or `from_node` escape hatches.
Those are only aliases for the same underlying leak.

The correct fix is not to hide `Node`.
The correct fix is to make the closed tree carrier the public `Widget` type and remove `Node` entirely from the authoring vocabulary.

### 3.2 `Widget<S>` forces app-state generics into every widget

Most built-in widgets do not need the application state type:

- `Text` does not care about `S`;
- `Row` does not care about `S`;
- `Image` does not care about `S`;
- `Container` does not care about `S`;
- `Button` stores an already-bound action descriptor, not a reducer generic over `S`.

Making every widget generic over `S` spreads application state through the entire authoring tree.
That increases compile-time surface area, makes examples noisier, and gives developers the wrong mental model.

In the new API, `GlobalState` appears only where it is explicitly requested:

```rust
let (ctx, view) = fission::build::current::<GlobalState>();
```

### 3.3 `AnyWidget` and `IntoWidget` create a second authoring model

`AnyWidget` and `IntoWidget` were intended to hide `Node`, but they introduced a second public concept for "something widget-like".
That made the API harder to teach and easier to misuse.

The standard Rust conversion traits already express the desired concept:

```rust
impl From<MyWidget> for Widget
```

Once the closed tree carrier is named `Widget`, there is no need for a custom conversion trait.

### 3.4 `build_node` is still a public Node API by another name

A method named `build_node` or a private trait that still appears in impls teaches developers that the real implementation target is a node conversion layer.
Even if it is technically hidden from common imports, it becomes visible in crate docs, source examples, and AI training context.

The public API must have one conversion shape:

```rust
impl From<MyComponent> for Widget
```

There must be no public or recommended method named `build_node`, `lowered`, `internal_node_widget`, or equivalent.

### 3.5 Builder-style child appending hurts complex trees

This style is not acceptable as a public authoring pattern:

```rust
Column::new()
    .child(Text::new("A"))
    .child(Text::new("B"))
    .child(Row::new().child(Button::new("Save", save)))
```

It hides hierarchy in a long chain and becomes difficult to read in real apps.
It also creates two ways to construct the same widget.

The new API keeps structure visible with public fields where they are stable, and constructors only where they improve correctness or hide internal representation details:

```rust
Column {
    children: widgets![
        Text::new("A"),
        Text::new("B"),
        Row {
            children: widgets![
                Button::new("Save", save),
            ],
            ..Default::default()
        },
    ],
    ..Default::default()
}
```

Optional configuration can be set with public fields or method chaining, depending on the widget.
Method chaining is acceptable for non-structural configuration because it does not obscure tree shape:

```rust
(
    Column {
        children: widgets![...],
        ..Default::default()
    }
)
    .spacing(16)
    .cross_axis_alignment(CrossAxisAlignment::Center)
```

---

## 4. Goals

### 4.1 API goals

The v2 authoring API must:

- remove `Node` from public authoring code;
- remove the public `Widget<S>` trait;
- avoid generic app-state parameters on ordinary built-in widgets;
- use standard `From<T> for Widget` for custom components;
- keep child fields ergonomic with `Vec<Widget>`;
- support retained local widget state with explicit syntax;
- preserve typed access to global app state and reducers;
- support scoped provider-style values and build context access;
- preserve deterministic lowering and testing;
- make examples and documentation use one canonical style.

### 4.2 Documentation goals

Examples in docs and in the repository must:

- show struct-based components;
- show `impl From<Component> for Widget`;
- use `widgets![...]` for child lists;
- allow dynamic child lists with normal `.map(...).collect::<Vec<Widget>>()`;
- avoid `.child().child()`;
- avoid `Node`;
- avoid `AnyWidget`;
- avoid `IntoWidget`;
- avoid hidden lowering helpers.

### 4.3 Compiler and shell goals

Shells and test tools must continue to:

- create the root `BuildCtx`;
- build a complete public `Widget` tree;
- lower `Widget` into Core IR deterministically;
- preserve action registration, resources, portals, animations, service registrations, and platform integration.

---

## 5. Non-Goals

This RFC does not add a second UI language or DSL.
It does not introduce a React-like hook system.
It does not retain arbitrary user component structs as runtime objects.
It does not allow closures to be stored in the widget tree.
It does not require separate `StatelessWidget` and `StatefulWidget` traits.
It does not preserve source compatibility with the old authoring API.

Custom low-level drawing and platform embedding remain supported through explicit first-class widgets such as `Canvas`, `CustomPaint`, `Image`, `Video`, `WebView`, and shell capabilities.
They are not provided through a generic `Node::Custom` escape hatch.

---

## 6. Normative Terminology

This RFC uses the following terms as public API concepts.

### 6.1 `Widget`

`Widget` is the closed public tree value.
It replaces the current public `Node` type.
It is an enum or equivalent closed representation owned by Fission.

A `Widget` is data.
It is not a trait.
It is not implemented by user types.
It is the normalized tree value that shell and core pipelines consume.

### 6.2 Built-in widget type

A built-in widget type is a Fission-provided struct such as `Text`, `Button`, `Column`, `Row`, `Image`, `Grid`, or `TextField`.
Built-in widget types convert into `Widget`:

```rust
impl From<Text> for Widget
impl From<Button> for Widget
impl From<Column> for Widget
```

### 6.3 Custom component

A custom component is a user-defined Rust struct that converts into `Widget`:

```rust
struct ProductCard {
    product: Product,
}

impl From<ProductCard> for Widget {
    fn from(card: ProductCard) -> Widget {
        // returns a Widget tree
    }
}
```

The docs may use the phrase "custom widget" for approachability, but the technical mechanism is `From<T> for Widget`.

### 6.4 `GlobalState`

`GlobalState` is app-wide state for one running app instance.
It stores domain/application data, not transient UI mechanics.
It is not process-global.
It is not a singleton.

### 6.5 `LocalWidgetState`

`LocalWidgetState` is retained state owned by a mounted widget identity.
It is appropriate for local UI state such as text field drafts, selected tabs, dropdown open state, animation handles, and panel expansion.

### 6.6 Build scope

A build scope is the active context in which `From<T> for Widget` conversions happen.
It contains access to the current `BuildCtx`, `View`, environment, local widget identity stack, provider stack, and runtime-owned build services.

### 6.7 Build handles

`BuildCtxHandle<S>` and `ViewHandle<S>` are scoped handles returned by:

```rust
let (ctx, view) = fission::build::current::<GlobalState>();
```

They are not raw Rust references.
They resolve against the active build scope when their methods are called.
Using them outside an active build pass is a programming error and must fail with a clear diagnostic.

---

## 7. Public API Shape

### 7.1 The `Widget` tree type

The public tree type is named `Widget`:

```rust
pub enum Widget {
    Text(Text),
    Button(Button),
    Column(Column),
    Row(Row),
    Container(Container),
    Image(Image),
    Grid(Grid),
    TextField(TextField),
    // all other built-in widget variants
}
```

The concrete representation may be optimized internally, but the public concept is a closed tree value named `Widget`.
No public type named `Node` exists in the authoring API.

### 7.2 Built-in widget conversion

Every built-in widget type implements `From<T> for Widget`:

```rust
impl From<Text> for Widget {
    fn from(text: Text) -> Widget {
        Widget::Text(text)
    }
}
```

Users normally do not call `Widget::Text(...)` directly.
They use `.into()` at the final boundary or rely on constructors/macros that accept `impl Into<Widget>`.

### 7.3 Custom component conversion

Custom components implement `From<Component> for Widget`:

```rust
struct Header {
    title: String,
}

impl From<Header> for Widget {
    fn from(header: Header) -> Widget {
        Row {
            children: widgets![
                Text::headline(header.title),
                Spacer::new(),
            ],
            ..Default::default()
        }
        .into()
    }
}
```

Users should implement `From<Component> for Widget`, not `Into<Widget> for Component`.
Rust automatically provides `Into<Widget>` when `From<Component> for Widget` exists.

### 7.3.1 Why this can use Rust's standard conversion traits

Earlier designs could not cleanly use `Into<Widget>` because `Widget` was a trait name and the real tree value was `Node`.
That forced custom traits such as `IntoWidget`.

In this design, `Widget` is the concrete closed tree value.
Therefore the standard Rust conversion pattern is enough:

```rust
impl From<MyComponent> for Widget
```

This is permitted by Rust's orphan rules because `MyComponent` is a local type.
The implementation target is concrete and non-generic.
No custom `IntoWidget` trait is required.

### 7.4 No public `Widget` trait

There is no public trait named `Widget`.
The name `Widget` belongs to the closed tree value.
This avoids the current confusion where `Widget<S>` is a trait but `Node` is the value actually consumed by the framework.

### 7.5 Child slots

Widgets with one child accept `impl Into<Widget>`:

```rust
impl Center {
    pub fn new(child: impl Into<Widget>) -> Self;
}

impl Container {
    pub fn new(child: impl Into<Widget>) -> Self;
}
```

Widgets with multiple children expose plain `Vec<Widget>` child fields or accept `Vec<Widget>` in constructors:

```rust
impl Column {
    pub fn new(children: Vec<Widget>) -> Self;
}

impl Row {
    pub fn new(children: Vec<Widget>) -> Self;
}
```

The stored child type is always `Widget`:

```rust
pub struct Column {
    pub children: Vec<Widget>,
    pub spacing: Length,
    pub alignment: CrossAxisAlignment,
}
```

This is the clean public API.
Fission must not make common dynamic UI harder by requiring an opaque list wrapper.

### 7.6 The `widgets![...]` macro

The `widgets![...]` macro constructs a `Vec<Widget>` while allowing heterogeneous child expressions without repeated `.into()` calls:

```rust
Column {
    children: widgets![
        Text::headline("Inbox"),
        SearchBox::new(),
        MessageList::new(messages),
    ],
    ..Default::default()
}
```

The macro expands conceptually to:

```rust
vec![
    Text::headline("Inbox").into(),
    SearchBox::new().into(),
    MessageList::new(messages).into(),
]
```

It must preserve source order exactly.
It must not perform runtime type inspection.
It must not evaluate child expressions more than once.
It is ergonomic sugar, not a special correctness mechanism.

Dynamic lists use normal Rust iterators:

```rust
Column {
    children: products
        .iter()
        .map(|product| {
            ProductRow::new(product)
                .key(product.id)
                .into()
        })
        .collect(),
    ..Default::default()
}
```

Keys are what preserve local retained widget state across reorder/filter/insert/remove operations.
Unkeyed dynamic children are valid, but their local state follows structural position.

### 7.6.1 Why `Vec<Widget>` works with retained local state

Retained local state does not require child expressions to be converted under a special list scope.
That would make normal Rust collection patterns awkward and would break common UI code that computes children from data.

Instead, local state is resolved by the runtime from the mounted widget tree:

```text
component type identity
+ explicit key/id when present
+ otherwise structural path
+ local state field identity
```

This is why plain `Vec<Widget>` is sufficient.
The vector preserves child order.
The mounted tree supplies structural position.
Explicit keys supply stable identity for dynamic/reorderable children.

The consequence is deliberate:

- static child vectors work without keys;
- `.map(...).collect::<Vec<Widget>>()` is valid and expected;
- keyed dynamic children retain their local state across insert/remove/reorder;
- unkeyed dynamic children retain by position;
- `widgets![...]` exists only to remove repetitive `.into()` calls.

### 7.7 No repeated child appending API

Built-in widgets must not expose repeated child appending methods as a public authoring pattern:

```rust
// Not allowed as public v2 API.
Column::new().child(a).child(b)
```

If an internal builder is needed for generated code, it must not be public, documented, or re-exported from the facade.

### 7.8 Optional configuration

Method chaining is allowed for non-structural configuration:

```rust
(
    Column {
        children: widgets![...],
        ..Default::default()
    }
)
    .spacing(12)
    .cross_axis_alignment(CrossAxisAlignment::Center)
```

This is acceptable because it modifies properties of the current widget rather than hiding child hierarchy.

---

## 8. Complete Minimal Counter App

This example is the canonical minimal app for the new API.
It intentionally uses local widget state, no global app state, and visible tree composition.
It does not introduce a separate root component because the counter itself is the whole app.
Platform/window metadata such as title, icon, size, splash screen, and close behavior belongs on the app/shell configuration or environment sync path, not in the normal widget tree.

```rust
use fission::prelude::*;

fn main() -> fission::Result<()> {
    DesktopApp::new(Counter {
        title: "Counter".to_owned(),
    })
    .title("Counter")
    .run()
}

#[fission_component]
struct Counter {
    title: String,

    #[local_state(default = 0)]
    count: i32,
}

#[fission_reducer(Decrement)]
fn decrement_count(count: &mut i32) {
    *count -= 1;
}

#[fission_reducer(Increment)]
fn increment_count(count: &mut i32) {
    *count += 1;
}

impl From<Counter> for Widget {
    fn from(counter: Counter) -> Widget {
        let (ctx, _) = fission::build::current::<()>();
        let count = counter.count();
        let decrement = ctx.bind_local(Decrement, count, reduce!(decrement_count));
        let increment = ctx.bind_local(Increment, count, reduce!(increment_count));

        Center::new(
            Column {
                children: widgets![
                    Text::headline(counter.title),
                    Text::body(format!("Count: {}", count.get())),
                    Row {
                        children: widgets![
                            Button::new("Decrement", decrement),
                            Button::primary("Increment", increment),
                        ],
                        spacing: 12,
                        ..Default::default()
                    },
                ],
                spacing: 16,
                main_axis_alignment: MainAxisAlignment::Center,
                cross_axis_alignment: CrossAxisAlignment::Center,
                ..Default::default()
            },
        )
        .into()
    }
}
```

This app demonstrates the required ergonomics:

- no `Node`;
- no `Widget<S>` trait;
- no `render()` convention;
- no `.child().child()`;
- no unnecessary root `App` wrapper;
- app/window metadata is configured on `DesktopApp`, not represented as UI widgets;
- no global state for a purely local counter;
- retained local widget state through `#[local_state]`;
- standard Rust `From<T> for Widget` conversion.

---

## 9. Global State

### 9.1 Purpose

`GlobalState` stores application/domain state for one app instance.
It is used when data must outlive a particular widget, be shared by distant parts of the tree, be persisted, be synchronized, or be tested through action traces.

Examples:

- authenticated user;
- shopping cart contents;
- selected workspace;
- current project/document;
- records fetched from a backend;
- user settings;
- route/domain navigation state;
- feature flags;
- offline sync state.

### 9.2 Global state is optional

Small apps do not need global state:

```rust
DesktopApp::new(Counter {
    title: "Counter".to_owned(),
})
.run()
```

Apps that need app-wide state provide it explicitly:

```rust
DesktopApp::new(AppRoot)
    .with_global_state(GlobalState::default())
    .run()
```

An `AppRoot` component is optional.
Use one only when the app needs top-level routing, providers, shell layout, or other real composition.
Do not add an empty root wrapper to simple apps.

### 9.3 Accessing global state

A component that needs global state requests scoped handles:

```rust
let (ctx, view) = fission::build::current::<GlobalState>();
```

Read global state through `view`:

```rust
let cart_count = view.select(|state| state.cart.items.len());
```

Mutate global state through actions and reducers:

```rust
let add_to_cart = ctx.bind(
    AddToCart(product_id),
    reduce!(on_add_to_cart),
);
```

Widgets store the resulting action descriptor, not the reducer closure.

### 9.3.1 Generated global-state views

Closure selectors are the minimal global-state read API.
For larger apps, Fission also provides generated read-only views over `GlobalState`:

```rust
#[derive(Default, Clone, FissionGlobalState)]
struct GlobalState {
    cart: CartState,
    session: SessionState,
}

#[derive(Default, Clone, FissionStateView)]
struct CartState {
    items: Vec<CartItem>,
}
```

The derive generates typed field lenses/views.
Authoring code can then read nested data without repeating closure paths:

```rust
let (_, view) = fission::build::current::<GlobalState>();

let app = view.global();
let items = app.cart().items().get();
let item_count = app.cart().items().map(|items| items.len()).get();
```

Generated views are read-only.
They do not mutate `GlobalState`.
Mutation still goes through actions and reducers.

The derive is deterministic:

- generated field identities use type path plus field name;
- skipped fields must be marked explicitly with `#[fission(skip_view)]`;
- nested field views are generated only for field types that derive `FissionStateView` or are covered by built-in collection/value views;
- no runtime string lookup or reflection is used.

### 9.4 Global state is not widget state

Global state must not be used for purely local UI mechanics.
Do not put dropdown open state, button press state, text field draft text, or animation progress in `GlobalState` unless there is a clear app-level reason.

The rule is:

```text
If losing the widget should lose the state, use LocalWidgetState.
If losing the widget must not lose the state, use GlobalState.
```

---

## 10. Local Widget State

### 10.1 Purpose

`LocalWidgetState` is retained UI state owned by a mounted widget identity.
It is used for state that belongs to one component and should not be part of the app/domain model.

Examples:

- text typed into a local search field;
- selected tab inside one panel;
- whether a disclosure section is expanded;
- hover/focus/pressed state;
- scroll position;
- local loading/error state for one widget;
- animation progress;
- a temporary form draft before save.

### 10.2 Declaring local state

Local retained fields are declared with `#[local_state]` inside a `#[fission_component]` struct:

```rust
#[fission_component]
struct SearchBox {
    placeholder: String,

    #[local_state(default = String::new())]
    query: String,
}
```

The component struct fields without `#[local_state]` are props.
They are recreated whenever the component is converted into `Widget`.

The fields marked `#[local_state]` are not stored as ordinary transient props.
They are moved into generated retained local state owned by the runtime.

### 10.3 Generated accessors

For each local state field, the macro generates an accessor with the same field name:

```rust
let query = search_box.query();
```

The accessor returns a typed state field handle:

```rust
StateField<String>
```

The exact type name is public, but users normally rely on inference.
`StateField::get` reads the current retained value.
Local state mutation uses explicit local reducers bound through `ctx.bind_local`.
State field handles do not produce button actions directly.

### 10.4 Reading and updating local state

State fields support reading through `get`.
Updates are made through named reducer functions bound with `ctx.bind_local`.
Interactive widgets receive action descriptors produced by binding, not local state closures.

```rust
#[fission_reducer(QueryChanged)]
fn set_query(query: &mut String, value: String) {
    *query = value;
}

let (ctx, _) = fission::build::current::<GlobalState>();
let query = search.query();
let query_changed = ctx.bind_local(
    QueryChanged(String::new()),
    query,
    reduce!(set_query),
);

TextField::new()
    .placeholder(search.placeholder)
    .value(query.get())
    .on_change(query_changed)
```

For manual updates:

```rust
#[fission_reducer(ClearQuery)]
fn clear_query(query: &mut String) {
    query.clear();
}

let (ctx, _) = fission::build::current::<GlobalState>();
let query = search.query();
let clear = ctx.bind_local(ClearQuery, query, reduce!(clear_query));

Button::new("Clear", clear)
```

The value returned by `ctx.bind_local` is represented as an action descriptor understood by the runtime.
The widget tree does not store arbitrary closures.
The reducer is a named function item generated by `#[fission_reducer(...)]` and registered during the active build pass.
Capturing closures must not be accepted for retained local state mutation.

### 10.5 Local state identity

Local state identity is resolved during mount/reconciliation from:

```text
component type identity
+ explicit key/id when present
+ otherwise structural path
+ local state field identity
```

The component type identity must be stable for the compiled crate version.
The field identity must be derived from the defining type and field name, not from source line number.

A local state accessor does not require the final structural path to be known at the exact moment the component is converted into `Widget`.
Instead, the accessor creates a typed local state field reference that carries:

```text
component type identity
+ optional explicit key/id
+ local state field identity
```

The runtime resolves that reference against the mounted widget tree.
If the component has an explicit key/id, that key participates in identity.
If no key/id is present, the runtime uses the component's structural position in the tree.

### 10.6 Keys

When stateful components appear in reorderable lists, the component must have a stable key:

```rust
ProductRow::new(product).key(product.id)
```

Without a key, local state follows structural position.
This is acceptable for static trees but incorrect for reorderable or filtered lists.

### 10.7 Disposal

When a mounted widget identity disappears from the tree, its local state becomes eligible for disposal.
The runtime may dispose it immediately after the frame or after a short retention window needed for transitions.
Disposal must be deterministic from the runtime's point of view.

### 10.8 Local state is not persistent storage

Local state should not be used as a database, cache, session store, or app model.
It is retained UI memory.
Persistence belongs in global state, services, jobs, storage capabilities, or server state.

---

## 11. `#[fission_component]`

### 11.1 Purpose

`#[fission_component]` marks a struct as a Fission component and enables local state fields.
It exists to make retained local widget state ergonomic without requiring developers to manually write hidden state structs, field IDs, and state accessors.

### 11.2 Props and local state fields

Given:

```rust
#[fission_component]
struct Counter {
    title: String,

    #[local_state(default = 0)]
    count: i32,
}
```

The macro treats `title` as a prop and `count` as local widget state.
The public struct constructor should accept only props unless the user explicitly opts into an advanced initializer for local state.
After macro expansion, struct literals and generated constructors expose only prop fields:

```rust
Counter {
    title: "Counter".to_owned(),
}
```

The local state field is accessed through `counter.count()`, not initialized as a normal struct field.

Conceptual expansion:

```rust
struct Counter {
    title: String,
    __fission_key: Option<WidgetKey>,
}

struct CounterLocalWidgetState {
    count: i32,
}

impl Counter {
    fn count(&self) -> StateField<i32> {
        fission::state::local_field::<CounterLocalWidgetState, i32>(
            LocalStateFieldId::new("crate::Counter::count"),
            || CounterLocalWidgetState { count: 0 },
            |state| &state.count,
            |state| &mut state.count,
        )
    }
}
```

The actual generated code may use more compact internal identifiers.
The semantics above are mandatory.

### 11.2.1 Local state defaults

`#[local_state]` supports deterministic default initialization:

```rust
#[local_state(default = 0)]
count: i32,

#[local_state(default = String::new())]
query: String,

#[local_state(default_with = make_initial_filter)]
filter: FilterState,
```

Defaults are evaluated only when the retained state field is first created for a mounted identity.
Defaults are not re-evaluated on every rebuild.
Defaults must be deterministic.
They must not read wall-clock time, random sources, network state, or other nondeterministic inputs.

### 11.3 Key support

The macro provides key support for local state identity:

```rust
Counter::new("Counter").key("main-counter")
```

The key is part of the component's retained identity.
Keys must be deterministic.
Keys must not depend on random values or wall-clock time.

### 11.4 No generated `render()` convention

The macro must not require or generate a public `render()` method.
The authoring shape remains:

```rust
impl From<Counter> for Widget {
    fn from(counter: Counter) -> Widget {
        // build tree here
    }
}
```

The macro may generate private helper methods and private state types.
It must not introduce an alternative public component lifecycle method.

### 11.5 Identity reference creation

The framework must know which component type and local field a local state accessor refers to.
It does not need the final structural tree path at the moment the accessor is called.
That path is resolved later during mount/reconciliation.

State accessors generated by `#[fission_component]` create typed local state field references from:

- component type identity;
- optional explicit key/id on the component;
- local state field identity.

The mounted tree supplies the structural path when no explicit key/id is present.

The public requirement is that this compiles and behaves correctly with only the struct macro:

```rust
#[fission_component]
struct Counter {
    #[local_state(default = 0)]
    count: i32,
}

impl From<Counter> for Widget {
    fn from(counter: Counter) -> Widget {
        let count = counter.count();
        Text::new(count.get().to_string()).into()
    }
}
```

Users must not have to annotate the `From` impl.
Users must not have to manually call `component::enter`.

### 11.6 Diagnostics

If a local state accessor is called outside an active build pass, Fission must fail with a diagnostic equivalent to:

```text
Fission local widget state was accessed outside an active widget build scope.
This usually means a component was converted into Widget outside DesktopApp/WebApp/test rendering.
```

---

## 12. Scoped Build Context

### 12.1 Shell-owned root context

Shells continue to create the root `BuildCtx` for each build pass.
The shell owns the root queues and registries for:

- action bindings;
- local state update bindings;
- resources;
- animations;
- portals;
- media registrations;
- service/job registrations;
- capability declarations;
- diagnostics and instrumentation.

### 12.2 Entering a build scope

Internally, shell code enters a build scope before converting the app root into `Widget`:

```rust
fission::build::enter::<GlobalState>(&mut build_ctx, &view, || {
    app_root.into()
})
```

This function is internal or advanced API.
Normal app authors do not call it.

### 12.3 Getting scoped handles

Authoring code can request handles:

```rust
let (ctx, view) = fission::build::current::<GlobalState>();
```

Apps without global state use `()` as the global state type.
Local-only components still use the same API shape:

```rust
let (ctx, _) = fission::build::current::<()>();
```

The return values are handles, not references:

```rust
pub struct BuildCtxHandle<S> { /* opaque */ }
pub struct ViewHandle<S> { /* opaque */ }
```

They are cheap to copy if needed.
They must not expose raw mutable references to the underlying `BuildCtx`.

### 12.4 Why handles are required

This API is not sound as raw references:

```rust
fn current<S>() -> (&mut BuildCtx<S>, &View<S>)
```

There is no input lifetime to bind the returned references to, and repeated calls could create multiple mutable aliases to the same `BuildCtx`.
Handles avoid that problem.
Each method call resolves against the current scope and borrows internally for the duration of that method call.

### 12.5 Use outside a build pass

If a handle is used outside an active build pass, Fission must fail with a clear diagnostic:

```text
Fission build context used outside an active build pass.
```

This is acceptable.
Handles are ergonomic build-time tools, not durable runtime objects.

### 12.6 Child scopes and provider overlays

A component may provide scoped values or a narrowed build context to descendants.
Children do not care whether a value comes from the root scope or a nearer provider.

However, child scopes must not isolate framework registrations from the shell.
Registrations must either:

- write through to the root registries; or
- be merged into the parent scope before the child scope exits.

This prevents actions, resources, portals, animations, and service declarations from being lost.

### 12.6.1 Scoped `BuildCtx` replacement

Fission supports scoped build-context replacement for advanced widgets and shells.
The nearest active build context is what `fission::build::current::<S>()` resolves.
This allows a parent to provide a specialized context to its descendants without changing the descendants' code.

The replacement must be a scoped context that participates in parent/root drainage.
It is not allowed to silently fork the build into an isolated context.

Required behavior:

- child code calls `build::current::<S>()` and receives handles for the nearest context;
- action/resource/animation/portal/service registrations made through that context are visible to the shell after the root build pass;
- scope exit merges or forwards all registrations deterministically;
- diagnostics identify the scope that produced invalid or duplicate registrations.

This gives advanced composition without breaking the shell-owned lifecycle.

### 12.7 Provider values

Provider-style values are supported through build scopes:

```rust
ThemeProvider::new(theme, child)
```

Conceptual implementation:

```rust
impl<C> From<ThemeProvider<C>> for Widget
where
    C: Into<Widget>,
{
    fn from(provider: ThemeProvider<C>) -> Widget {
        fission::build::provide(provider.theme, || provider.child.into())
    }
}
```

Descendants can read the nearest provider value:

```rust
let theme = fission::build::read::<Theme>();
```

Provider child conversion must be deferred until the provider scope is active when descendants need that provider during conversion.
This means provider widgets store children as `C: Into<Widget>` rather than preconverted `Widget` for provider-sensitive slots.
Plain layout widgets still store `Vec<Widget>` children.

---

## 13. Actions And Reducers

### 13.1 Global state mutations

Global state mutations continue to use typed actions and reducers.
A component binds an action during build:

```rust
let (ctx, _) = fission::build::current::<GlobalState>();

let save = ctx.bind(
    SaveDocument(document_id),
    reduce!(save_document),
);
```

The returned value is an action descriptor that can be stored in `Widget`.
The reducer function itself is registered in the build context and is not stored in the tree.

### 13.2 Local state mutations

Local state updates use the same underlying action descriptor mechanism.
The difference is that the target is a retained local widget state field rather than `GlobalState`.

```rust
#[fission_reducer(Increment)]
fn increment_count(count: &mut i32) {
    *count += 1;
}

let (ctx, _) = fission::build::current::<GlobalState>();
let count = counter.count();
let increment = ctx.bind_local(Increment, count, reduce!(increment_count));

Button::new("Increment", increment)
```

The reducer function must not be stored in the `Widget` tree.
It is registered during build and represented in the tree by the returned action descriptor.
For the common case, define it with `#[fission_reducer(ActionName)]` so the action type and reducer stay together.
For local state bindings, the binding identity must include the local state target reference.
Two mounted instances of the same component may bind the same action type, such as `Increment`, but dispatch must update only the local state for the widget instance that produced the action descriptor.

### 13.3 No closures in the widget tree

The existing determinism rule remains:

```text
Widget trees do not store arbitrary closures.
```

Closures may appear in authoring syntax only where they are consumed during the active build pass and converted into deterministic descriptors or registered handlers.

---

## 14. Lowering Pipeline

### 14.1 Authoring output

The authoring layer outputs a public `Widget` tree.
This tree replaces the old public `Node` tree.

```text
custom structs + built-in constructors
    -> Widget
    -> Core IR
    -> layout
    -> display list / platform output
```

### 14.2 Deterministic lowering

`Widget` lowers into Core IR through framework-owned lowering code.
Custom components do not lower directly.
They compose built-in widgets and disappear before lowering.

### 14.3 Low-level extension points

If an app needs custom drawing, custom hit testing, or native embedding, it must use explicit first-class widgets designed for that purpose, for example:

- `Canvas`;
- `CustomPaint`;
- `ShaderSurface`;
- `PlatformView`;
- `WebView`;
- `Video`;
- `Scene3D`.

This RFC rejects a generic public `Node::Custom` lowering escape hatch.
Such escape hatches make the authoring model unclear and weaken deterministic guarantees.

---

## 15. Lifecycle

### 15.1 Root build lifecycle

For each build pass:

1. The shell creates or reuses runtime state for the app instance.
2. The shell creates a root `BuildCtx` for this pass.
3. The shell creates a `View` over `GlobalState`, environment, runtime inputs, and theme.
4. The shell enters a build scope.
5. The app root converts into `Widget`.
6. Fission lowers the `Widget` tree into Core IR.
7. Layout, paint, semantics, accessibility, and platform integration proceed from Core IR.
8. The shell drains build registrations from `BuildCtx`.
9. Local widget state retention/disposal is reconciled against mounted identities.

### 15.2 Component lifecycle

A component struct is transient.
It can be recreated every build.
Props are ordinary fields on that transient struct.
Local widget state is retained by the runtime, not by the transient struct.

### 15.3 State lifetime table

| State kind | Owner | Lifetime | Mutation path | Examples |
| --- | --- | --- | --- | --- |
| Props | Parent component | One conversion/build | Parent passes new value | title, product, callback descriptor |
| LocalWidgetState | Runtime retained widget store | Mounted widget identity | `ctx.bind_local` + local state field reducer | query text, selected tab, expanded row |
| GlobalState | App instance | App/session lifetime | typed action + reducer | cart, user, document, settings |
| Service/job state | Service/job runtime | Service/job lifetime | service/job protocol | sync, downloads, background work |
| Persistent state | Storage/backend | External lifetime | explicit storage/service API | database rows, session tokens |

---

## 16. Stateless And Stateful Components

Fission does not need separate `StatelessWidget` and `StatefulWidget` public traits.

A stateless component is simply a struct with no `#[local_state]` fields:

```rust
struct ProductTitle {
    name: String,
}

impl From<ProductTitle> for Widget {
    fn from(title: ProductTitle) -> Widget {
        Text::headline(title.name).into()
    }
}
```

A stateful component is a struct with one or more `#[local_state]` fields:

```rust
#[fission_component]
struct SearchBox {
    #[local_state(default = String::new())]
    query: String,
}
```

Both use the same conversion pattern.
This avoids unnecessary ceremony while preserving clear concepts in the documentation.

---

## 17. Full Example With Global State

```rust
use fission::prelude::*;

#[derive(Default, Clone, FissionGlobalState)]
struct GlobalState {
    cart: CartState,
}

#[derive(Default, Clone, FissionStateView)]
struct CartState {
    items: Vec<ProductId>,
}

#[fission_reducer(AddToCart)]
fn add_to_cart(state: &mut GlobalState, product_id: ProductId) {
    state.cart.items.push(product_id);
}

struct ProductCard {
    product: Product,
}

impl From<ProductCard> for Widget {
    fn from(card: ProductCard) -> Widget {
        let (ctx, view) = fission::build::current::<GlobalState>();

        let in_cart = view
            .global()
            .cart()
            .items()
            .map(|items| items.contains(&card.product.id))
            .get();
        let add = ctx.bind(
            AddToCart(card.product.id),
            reduce!(add_to_cart),
        );

        Card::new(
            Column {
                children: widgets![
                    Image::network(card.product.image_url),
                    Text::headline(card.product.name),
                    Text::body(card.product.description),
                    Button::primary(
                        if in_cart { "Added" } else { "Add to cart" },
                        add,
                    )
                    .enabled(!in_cart),
                ],
                ..Default::default()
            },
        )
        .into()
    }
}
```

This example shows the intended split:

- product props are passed into `ProductCard`;
- cart contents live in `GlobalState`;
- the card reads global state through `view`;
- mutation happens through an action/reducer;
- no built-in widget is generic over `GlobalState`.

---

## 18. Failed Alternatives

### 18.1 Keep `Widget<S>` and hide `Node` behind `IntoWidget`

This approach kept the old trait shape:

```rust
pub trait Widget<S> {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl IntoWidget<S>;
}
```

It failed because child slots still needed a concrete stored type.
That forced either `Node`, `AnyWidget`, or another erased wrapper to appear throughout the implementation.
It did not actually remove the old model.

### 18.2 Add `AnyWidget`

`AnyWidget` introduced another erased widget carrier.
It created clone/lifetime/performance questions and still required conversion back to the underlying tree representation.
It also added another public type for developers and AI tools to misuse.

Rejected.

### 18.3 Add `build_node`

`build_node` made the old tree conversion explicit in another method.
It looked internal but still taught the wrong API shape.
It was effectively the old `Node` API under a different name.

Rejected.

### 18.4 Mirror every widget with a `<Widget>Node` type

Creating types such as `ContainerNode`, `TextNode`, or similar duplicates the widget model instead of simplifying it.
It makes the implementation larger, creates mapping drift, and still keeps two concepts where one should exist.

Rejected.

### 18.5 Retain arbitrary user widget objects directly

Keeping arbitrary user component objects in memory appears attractive but does not solve type erasure.
A heterogeneous tree still needs one concrete child slot type.
Retaining arbitrary structs also complicates lifetimes, object safety, diffing, serialization, testing, and deterministic inspection.

Rejected.

### 18.6 Use `Box<dyn Widget<S>>` children

Trait-object widget trees create object-safety issues, lifetime complexity, dynamic dispatch in the authoring tree, and generic state spread.
They also make deterministic inspection harder than a closed tree value.

Rejected.

### 18.7 Keep repeated `.child()` builders

Repeated child append chains are hard to read in complex trees and provide no meaningful capability that direct tree composition lacks.
They also encourage examples that obscure hierarchy.

Rejected as public API.

### 18.8 Split public `StatelessWidget` and `StatefulWidget` traits

A split trait model adds ceremony without solving a Fission-specific problem.
The presence or absence of `#[local_state]` already communicates the distinction.

Rejected.

---

## 19. Required Removals

The v2 implementation must remove or make private all of the following from the public facade and generated docs:

- `Node`;
- `Widget<S>` trait;
- `AnyWidget`;
- `IntoWidget`;
- `build_node`;
- `into_node`;
- `from_node`;
- `internal_node_widget`;
- `lowered` constructors that accept the old tree carrier;
- public `.child(...)` append APIs for multi-child widgets;
- public APIs that expose `Vec<Node>`, `Box<Node>`, `Option<Node>`, or equivalent aliases.

Internal implementation modules may use private names, but those names must not become part of the public authoring API, examples, or generated crate documentation.

---

## 20. Migration Rules

### 20.1 Old custom widget

Old:

```rust
impl<S: AppState> Widget<S> for Counter {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> impl IntoWidget<S> {
        Column::new()
            .child(Text::new("Counter"))
            .child(Button::new("Increment"))
    }
}
```

New:

```rust
#[fission_reducer(Increment)]
fn increment(state: &mut GlobalState) {
    state.count += 1;
}

impl From<Counter> for Widget {
    fn from(counter: Counter) -> Widget {
        let (ctx, _) = fission::build::current::<GlobalState>();
        let increment = ctx.bind(Increment, reduce!(increment));

        Column {
            children: widgets![
                Text::new("Counter"),
                Button::new("Increment", increment),
            ],
            ..Default::default()
        }
        .into()
    }
}
```

### 20.2 Old app state access

Old:

```rust
fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> impl IntoWidget<AppState>
```

New:

```rust
let (ctx, view) = fission::build::current::<GlobalState>();
```

### 20.3 Old local UI state stored in app state

Old:

```rust
struct AppState {
    search_query: String,
}
```

New, when the query is only local UI draft state:

```rust
#[fission_component]
struct SearchBox {
    #[local_state(default = String::new())]
    query: String,
}
```

Keep it in `GlobalState` only if it is part of the app model, route state, persistence, sync, or shared behavior.

---

## 21. Testing Requirements

The implementation must include tests for the following.

### 21.1 Compile-time API tests

- custom component with `impl From<T> for Widget` compiles;
- built-in widgets are usable without specifying `GlobalState`;
- `widgets![...]` accepts heterogeneous children;
- examples do not import or name `Node`;
- examples do not implement `Widget<S>`;
- repeated `.child()` public APIs are not available.

### 21.2 Local state tests

- `#[local_state]` field persists across rebuilds for the same identity;
- local state is distinct for two sibling components of the same type;
- keyed local state survives list reordering;
- unkeyed local state follows structural position;
- removed component state is disposed according to runtime policy;
- local state access outside build scope reports a clear diagnostic.

### 21.3 Global state tests

- `build::current::<GlobalState>()` returns usable handles during build;
- global state selectors read the current app state;
- generated global-state views read nested fields without runtime string lookup;
- reducer-bound actions update global state;
- built-in widgets store action descriptors, not reducer closures.

### 21.4 Build scope tests

- nested provider scopes resolve nearest provider value;
- child scopes do not lose action/resource/animation registrations;
- using a captured build handle outside the build pass fails clearly;
- multiple calls to `build::current::<S>()` do not create unsound mutable aliases.

### 21.5 Lowering and shell tests

- a built `Widget` tree lowers to Core IR deterministically;
- shell build passes still drain actions, resources, animations, portals, and media registrations;
- test harness can inspect `Widget` and Core IR without `Node`;
- screenshots/golden tests continue to operate against Core IR/display lists.

---

## 22. Documentation Requirements

Documentation must be updated in the same change set as the API refactor.

Required updates:

- root README;
- facade crate README;
- getting started guide;
- counter example guide;
- state guide;
- actions/reducers guide;
- widget reference introduction;
- testing guide;
- examples README files;
- AI/tooling guidance if present.

Documentation must consistently use:

```rust
impl From<MyComponent> for Widget
```

and must not teach:

```rust
impl Widget<S> for MyComponent
```

The docs must explain:

- why `Widget` is a value, not a trait;
- why custom components implement `From<T> for Widget`;
- when to use `LocalWidgetState`;
- when to use `GlobalState`;
- why handles are returned by `build::current` instead of raw references;
- why repeated `.child()` construction is not part of the public API.

---

## 23. Implementation Strategy

The implementation should be done as one breaking refactor, not as a compatibility layer.
Compatibility shims are explicitly rejected because examples and crate docs must become the gold-standard API surface.

Recommended order:

1. Rename the current authoring tree carrier from `Node` to `Widget` internally and publicly.
2. Remove or privatize the current `Widget<S>` trait.
3. Convert built-in widgets to store `Widget` in child slots.
4. Replace public child append APIs with `Vec<Widget>` child fields and constructors only where useful.
5. Add `widgets![...]` as ergonomic `Vec<Widget>` construction.
6. Add scoped build context entry/current APIs with handles.
7. Add `GlobalState` naming and shell startup support.
8. Add generated global-state read views.
9. Add `#[fission_component]` and `#[local_state]` macro support.
10. Add retained local widget state storage keyed by widget identity.
11. Update actions/reducers to bind through scoped handles.
12. Update shells and tests to build from `Into<Widget>` app roots.
13. Update all examples and docs.
14. Remove all old authoring names from public exports.
15. Add compile-fail and integration tests to prevent old API reintroduction.

No step should introduce `AnyWidget`, `IntoWidget`, `build_node`, or a public compatibility alias for `Node`.

---

## 24. Compatibility Policy

This RFC defines a hard break.
Source compatibility with the old authoring API is not preserved.
The version implementing this RFC must be treated as an API-breaking release.

The reason is deliberate: compatibility aliases would keep the old patterns visible in code, docs, crate metadata, and AI-generated examples.
That defeats the point of the refactor.

---

## 25. Acceptance Criteria

The refactor is complete only when all of the following are true:

- `rg "\bNode\b" crates examples documentation` finds no public authoring API usage;
- `rg "Widget<" crates examples documentation` finds no public authoring API usage;
- `rg "build_node|into_node|from_node|AnyWidget|IntoWidget" crates examples documentation` finds no public authoring API usage;
- every built-in widget child slot stores `Widget`, not `Node` or an erased wrapper;
- all examples use `impl From<T> for Widget` for custom components;
- all multi-child examples use `Vec<Widget>` child fields or `Vec<Widget>` constructors with `widgets![...]` where it improves readability;
- no public `.child()` appending API remains;
- local widget state survives rebuilds and keyed reorder tests;
- global state actions/reducers continue to work;
- desktop, web, mobile, terminal, static site, and server shells can build app roots into `Widget`;
- workspace tests pass;
- live widget behavior tests cover local state, global state, actions, and layout.

---

## 26. Final Design Position

The final model is intentionally simple:

```text
Rust struct props
+ optional #[local_state] fields
+ impl From<Component> for Widget
+ widgets![...] child lists
+ scoped build handles when needed
= public Widget tree
```

There is no second authoring tree type.
There is no public widget trait.
There is no state generic on every widget.
There is no child builder chain.
There is no compatibility path back to the old API.

This makes Fission's authoring model easier to teach, easier to document, easier for AI tools to generate correctly, and harder to accidentally route back through the architectural problems this RFC is intended to remove.
