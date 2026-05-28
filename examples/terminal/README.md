# Terminal

Terminal demonstrates the terminal widget running inside a Fission desktop window. It launches a local shell, polls the terminal session for output, and renders a compact terminal frame with window chrome.

Use this example when you want to study host-backed terminal sessions, timer resources, and periodic redraws without opening the larger Fission CLI UI.

## Run it

```bash
cargo run -p terminal
```

## What to look at

- [`src/main.rs`](src/main.rs) contains the full app.
- `TerminalExampleState` stores the current working directory, active terminal session, and redraw epoch.
- `StartTerminal` and `PollTerminal` show manual action registration for reducers that need access to `ReducerContext`.
- `ctx.resources.timer(...)` shows how a `TimerResource` drives polling.
- `TerminalView` and `TerminalSession` are the host-backed terminal pieces enabled by the `terminal-widget` feature in [`Cargo.toml`](Cargo.toml).

## Features exercised

- `TerminalSession::spawn(...)` for launching a shell.
- `TerminalView` rendering inside normal Fission layout.
- Timer resources for periodic polling.
- Manual `Action` implementations for explicit action identity.
- Desktop shell integration for a host-backed widget.

## Learning path

Read `start_terminal` first, then `poll_terminal`, then the timer registration in `TerminalExampleApp::build`. That flow shows how the app starts a host resource, checks for changes, and asks Fission to rebuild only when the terminal has new output.
