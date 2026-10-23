# fission-compose-macros

Procedural macros for Fission Compose. This crate parses component declarations and lowers them to ordinary Fission widget construction, typed state access, retained local fields, action registration and stable identity.

Application code should not depend on this implementation crate directly. Use the [`fission`](https://crates.io/crates/fission) facade and `fission::prelude::*`; the facade exposes `fission_component!` and `state_scope!` with the matching runtime support from `fission-compose`.

The macro deliberately leaves Rust responsible for names and types:

- component and widget names must be in normal Rust scope;
- named properties become direct public struct fields with `Default` filling omitted fields;
- positional properties call an existing `new(...)` constructor;
- ordinary expressions, `if`, `match` and keyed `for` children keep their Rust meaning;
- inline handlers lower to typed reducer registrations, while existing action values pass through unchanged;
- explicit state scopes are checked through generated Rust implementations.

The crate does not inspect types in another crate, generate widget builders or maintain a component registry. That boundary is intentional: built-in, application and third-party widgets all follow one authoring contract, and compiler errors remain the authority when a symbol or field does not exist.

See the [Compose guide](https://fission.rs/compose/) for the public syntax and the [`compose` example](https://github.com/fission-ui/fission/tree/main/examples/compose) for routes, bindings, state scopes, internationalisation and tests.

## Development

Parser and expansion tests live alongside the macro. Changes should preserve stable identity when declarations are reordered and reject unkeyed dynamic children or ambiguous handler identities.

## Licence

Apache-2.0.
