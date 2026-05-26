# fission-shell-terminal

Terminal shell backend for Fission applications.

`fission-shell-terminal` renders Fission widget trees into an interactive terminal user interface on Windows, macOS, and Linux. It is used by the `fission ui` command and can also be used by applications that need a first-class terminal UI.

Application developers normally enable it through the facade crate:

```toml
[dependencies]
fission = { version = "0.1.1", features = ["terminal-shell"] }
```

## What it contains

- A terminal renderer for Fission nodes and layout output.
- Keyboard and mouse input handling through the terminal backend.
- Focus traversal, buttons, menus, dialogs, scroll regions, and log-style scrollback surfaces.
- Theme support, including dark and light terminal presentations.

## Design notes

The terminal shell preserves the Fission architecture: applications still define state, actions, reducers, and widgets. The shell chooses a terminal-appropriate lowering path and reports unsupported visual features rather than inventing a separate app model.

## Documentation

See [Building terminal user interfaces](https://fission.rs/docs/guides/terminal-user-interfaces/) and the `fission ui` developer tool for a production example.

## License

MIT
