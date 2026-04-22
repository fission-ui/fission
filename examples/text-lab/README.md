# text-lab

`text-lab` is an isolated harness for text and text-input behavior.

It intentionally exercises:

- single-line text input,
- multiline text input,
- combobox wrappers,
- menu/dropdown overlays,
- and text input inside a modal + focus scope.

## Run

```bash
cargo run -p text-lab
```

## Run with latency trace

```bash
FISSION_TEXT_TRACE=1 cargo run -p text-lab
```

This emits per-input trace lines to stderr in the form:

- `handle_ms`: event handling time in runtime/controllers
- `effects_ms`: pending effects processing time
- `queue_ms`: delay until first present after handling
- `total_ms`: end-to-end from input event start to present

## Optional knobs

```bash
FISSION_MAX_FPS=60 FISSION_TEXTINPUT_BLINK=1 FISSION_TEXT_TRACE=1 cargo run -p text-lab
```

`FISSION_TEXTINPUT_BLINK_MS` can be used to tweak caret blink period.
