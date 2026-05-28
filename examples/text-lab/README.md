# Text Lab

Text Lab is an isolated harness for text input, focus, overlay, and latency behavior. It intentionally collects several text-heavy controls in one place so regressions are easy to reproduce.

Use this example when you are testing text entry, caret behavior, menus, combobox suggestions, modals, focus scopes, or input latency.

## Run it

```bash
cargo run -p text-lab
```

## Run with latency tracing

```bash
FISSION_TEXT_TRACE=1 cargo run -p text-lab
```

Trace output includes:

- `handle_ms`: event handling time in runtime/controllers.
- `effects_ms`: pending effects processing time.
- `queue_ms`: delay until first present after handling.
- `total_ms`: end-to-end time from input event start to present.

Optional knobs:

```bash
FISSION_MAX_FPS=60 FISSION_TEXTINPUT_BLINK=1 FISSION_TEXT_TRACE=1 cargo run -p text-lab
```

`FISSION_TEXTINPUT_BLINK_MS` can tune the caret blink period.

## What to look at

- [`src/main.rs`](src/main.rs) contains the full harness.
- `TextLabState` keeps all field values, menu state, modal state, and status output.
- The `SetSingleLine`, `SetMultiline`, `SetInlineCombobox`, and modal reducers show how text controls write into app state.
- The `filtered_suggestions` helper shows the data side of combobox suggestions.
- The modal section demonstrates text inputs inside `Modal` and `FocusScope`.

## Features exercised

- Single-line and multiline `TextInput`.
- `Combobox`, `MenuButton`, `MenuItem`, and modal overlays.
- `FocusScope` and explicit text-input node identifiers.
- Reducer-driven form state.
- Text latency tracing through environment variables.

## Learning path

Start with `TextLabState`, then follow one field such as `single_line`: the reducer updates the state, the text input receives the action id, and the build method reflects the latest value. Repeat that path for the modal fields to understand focus and overlay behavior.
