# Compose example

This is the reference application for Fission's recommended Compose authoring
API. It is deliberately larger than a counter: the app demonstrates how concise
component syntax scales across routes, reusable components and state scopes
without replacing Fission's existing runtime model.

The example covers:

- global application state and retained component-local state;
- inline reducers for local behaviour and reusable named reducers for shared
  domain transitions;
- one-way properties and two-way bindings, including explicit event bindings;
- conditional, matched and keyed repeated children;
- Fission's native `Router`, route parameters and not-found handling;
- scoped editing of an entity within the root state;
- translated user-facing text and design-system tokens;
- focused component, binding, routing and reducer-level tests.

Run the desktop application from the repository root:

```bash
cargo run -p compose-example
```

Run its focused tests with:

```bash
cargo test -p compose-example
```

Start with `src/lib.rs`, then follow the route components into their feature
modules. Imported built-in and application widgets use the same Compose syntax;
there is no registration table or Compose-specific widget catalogue. Low-level
custom widgets can still implement `From<YourWidget> for Widget` and participate
in the same tree.

The public guide at [fission.rs/compose](https://fission.rs/compose/) explains
the syntax and the boundary between inline component behaviour, reusable
reducers, jobs and effects.

## Licence

Apache-2.0
