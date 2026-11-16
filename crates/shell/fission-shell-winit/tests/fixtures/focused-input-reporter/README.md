# Focused input reporter fixture

This private crate is an executable test fixture for `fission-shell-winit`. It opens a small Fission desktop application whose focused text input reports host keyboard and text-editing behaviour to the shell integration tests.

It is intentionally its own workspace so test processes can build and launch it as an external application without inheriting the repository workspace configuration. It is not published, is not an example application and is not part of Fission's public API.

The fixture depends on the local `fission` facade with the `desktop` feature. Keep its behaviour deterministic and its interface aligned with the tests that launch it; product features and general demonstrations belong under `examples/` instead.

Run it only through the focused-input shell tests unless diagnosing the fixture itself. Those tests own process startup, interaction and cleanup.

## Licence

Apache-2.0, as part of the Fission repository.
