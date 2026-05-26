# fission-test

Headless testing framework for Fission widgets.

`fission-test` lets you build, lay out, render, and interact with Fission widget trees without
creating a window or GPU context. It provides `HeadlessApp` for lightweight lifecycle tests and
`TestDriver` for full interaction-driven tests -- tapping buttons, typing text, scrolling, and
asserting on visible output -- all running in-process with a mock text measurer and a
`TestRenderer` that captures the display list.

## Key types

| Type | Description |
|------|-------------|
| `HeadlessApp<S>` | Owns an `AppState`, root widget, layout engine, and mock text measurer. Call `tick()` to advance the clock. |
| `TestDriver<S>` | High-level driver wrapping a `TestHarness`. Provides query methods (`find_text`, `find_role`, `get_all_visible_text`) and interaction methods (`tap_text`, `type_text`, `press_key`, `scroll_down`, `scroll_to_text`). |
| `TestRenderer` | A no-op `Renderer` implementation that stores the last `DisplayList` for inspection. |
| `TextMatch` | A query result containing the matched text, bounding rect, and node ID. |
| `SemanticMatch` | A query result containing the matched accessibility role, label, bounds, and node ID. |

## Usage example

```rust
use fission_test::{TestDriver, TestHarness};

let harness = TestHarness::new(MyState::default(), MyWidget);
let mut driver = TestDriver::new(harness);

driver.set_viewport(800.0, 600.0);
driver.pump()?;

// Assert text is rendered
driver.assert_text_visible("Hello, world!");

// Simulate a tap on a button
driver.tap_text("Submit")?;

// Verify state change
driver.assert_text_visible("Submitted!");
```

## Environment variables

| Variable | Effect |
|----------|--------|
| `FISSION_TEST_USE_MOCK_MEASURER=1` | Force the mock text measurer (10 px per char, 20 px line height). |
| `FISSION_TEST_MEASURER=mock` | Same as above. |

## Status

**Supported** -- actively used for widget and integration tests across the Fission workspace.

## License

MIT -- see the [Fission repository](https://github.com/worka-ai/fission) for full documentation.
