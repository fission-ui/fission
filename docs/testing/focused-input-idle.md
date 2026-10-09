# Focused input idle CPU verification

Issue [#256](https://github.com/fission-ui/fission/issues/256) reports high CPU on Ubuntu while a basic text input is focused. The regression tests separate caret paint invalidation from paragraph layout. This manual probe also checks native scheduling, renderer selection, typing, a button action, and an injected IME preedit/commit.

## Build the native fixture on Linux

From the repository root, generate a standalone app:

```sh
cargo run -p cargo-fission --bin fission -- init /tmp/fission-idle-app --name fission-idle-app --local-path "$PWD"
```

Replace `/tmp/fission-idle-app/src/app.rs` with:

```rust
use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
    pub title: String,
}

impl GlobalState for CounterState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

#[fission_reducer(SetTitle)]
fn set_title(state: &mut CounterState, ctx: &mut ReducerContext<CounterState>) {
    if let Some(change) = ctx.input.text_change() { state.title = change.new_text.clone(); }
}

#[derive(Clone)]
pub struct CounterApp;

impl From<CounterApp> for Widget {
    fn from(_: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let increment = with_reducer!(ctx, Increment, on_increment);

        Column {
            gap: Some(16.0),
            children: vec![
                TextInput {
                    id: Some(WidgetId::explicit("cpu-title")),
                    semantics_identifier: Some("cpu.title".into()),
                    value: view.state().title.clone(),
                    placeholder: Some("Title".into()),
                    on_input: Some(with_reducer!(ctx, SetTitle, set_title)),
                    ..Default::default()
                }.into(),
                Text::new(format!("Count: {}", view.state().count)).size(28.0).into(),
                Button {
                    on_press: Some(increment),
                    child: Some(Text::new("Increment").into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()

    }
}

```

Retain the generated manifest, entry point, and `fission.toml`. New projects optimize dependencies for interactive debug builds. Existing apps can use the same setting in their `Cargo.toml`:

```toml
[profile.dev.package."*"]
opt-level = 1
```

Merge this with an existing package-profile table if present. The first dependency build can take longer; Cargo reuses those optimized dependencies during subsequent app edits. Application code retains its ordinary development profile. Release settings are unaffected.

Build the fixture:

```sh
CARGO_TARGET_DIR=/tmp/fission-idle-app/target cargo build --manifest-path /tmp/fission-idle-app/Cargo.toml
```

## Run the probe

Use a Linux graphical session or Xvfb with a working graphics adapter. Keep other build workloads stopped during sampling. Run from the repository root:

```sh
FISSION_IDLE_BINARY=/tmp/fission-idle-app/target/debug/fission-idle-app \
FISSION_IDLE_OUTPUT=/tmp/fission-idle-results/auto \
cargo test --locked -p fission-shell-winit --test focused_input_idle -- --ignored --nocapture
```

The test owns and cleans up its application process. It warms each state for two seconds, then samples ten untouched seconds for: unfocused input, focused input, and button focus. No test-control requests are made during those sampling intervals. CPU time comes from the application's `/proc/<pid>/stat` user/system ticks, divided by actual elapsed time; 100% means one CPU core. A reported zero means less than the measurement resolution, rather than a guarantee of no instructions executed.

The probe fixes the blink period at 530 ms, records frame-trace counts, and rejects sustained excess redraws. It then verifies typing, the counter action, and an injected IME commit. It writes CPU results, a renderer/frame log, and a focused screenshot into the output directory. Injected IME coverage does not replace testing the desktop's actual input-method integration.

Repeat with distinct output directories and these additional environment settings:

| Setting | Purpose |
| --- | --- |
| `FISSION_RENDERER=native-vello-gpu` | Verify an explicit GPU request still selects the GPU path. |
| `FISSION_RENDERER=software` | Compare the existing native CPU rasterizer directly. |
| `FISSION_IDLE_BLINK=0` | Verify that an untouched focused input stops redrawing without caret blinks. |

For the default fixture, expect approximately 19 focused caret redraws in ten seconds and no sustained redraws after focus moves to the button. Hardware GPU adapters should retain GPU rendering in automatic mode. CPU graphics adapters should choose `native-software-upload`, with `fallback_reason=cpu_adapter` on Linux. Explicit renderer requests should be honored.

## Interpretation

Record the Fission commit, build profile, Ubuntu/session type, CPU/GPU, startup `renderer:` line, and viewport alongside the results. Compare before and after using the same fixture and Cargo profile. Check screenshots and input behavior as well as CPU.

A software GPU adapter such as llvmpipe executes GPU shaders on the CPU. In the isolated Linux/X11 investigation, that path dominated focused idle cost. Caret layout invalidation was independently reproduced and corrected, although its removal alone did not produce a measurable CPU reduction in this small fixture. Dependency optimization also affects debug-build costs. These measurements do not establish the reporter's adapter or the behavior of physical Ubuntu hardware and Wayland.

## Recorded investigation results

The initial investigation used Debian 12 in an ARM64 Docker VM, Xvfb/X11, an 800×600 window, llvmpipe (LLVM 15.0.6), and a standalone debug app. Each value below is one ten-second process-CPU sample; it is diagnostic evidence rather than a hardware-independent performance target.

| Configuration | Focused CPU, one core | Redraws / 10 s |
| --- | ---: | ---: |
| Before: automatic GPU path, unoptimized dependencies | 36.7% | 19 |
| Patched: automatic native CPU path, unoptimized dependencies | 17.7% | 19 |
| Patched: explicit GPU path, unoptimized dependencies | 37.3% | 19 |
| Patched: automatic CPU path, generated dependency optimization | 4.3–4.4% | 19 |
| Patched: blinking disabled, unoptimized dependencies | Below tick resolution | 0 |

The unfocused and button-focused phases had zero redraws and no measurable CPU delta. The committed native probe passed the counter and Unicode IME assertions. Visual inspection found matching content; the GPU/CPU screenshot comparison changed 1,076 of 480,000 pixels, with a maximum RGB-channel difference of 2/255 and unchanged alpha, consistent with small antialiasing differences.

The package-profile change improves new project defaults. Existing projects retain their existing manifests and can adopt the setting above. The CPU-adapter selection and caret invalidation fixes apply through runtime updates. Confirmation on the reporter's version, Ubuntu hardware, renderer, and session remains outstanding; keep #256 open until that case is verified.
