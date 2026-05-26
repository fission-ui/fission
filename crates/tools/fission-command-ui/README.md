# fission-command-ui

Terminal UI application for the `fission` command.

`fission-command-ui` implements `fission ui`, a terminal interface for the same developer workflow exposed by the traditional CLI commands. It is a real Fission app built with the terminal shell.

## What it contains

- Fission widget screens for setup, project status, run/build/test workflows, logs, settings, and command confirmation dialogs.
- Non-blocking command execution so long-running operations do not freeze the terminal UI.
- Keyboard and mouse navigation through the terminal shell.
- Log tabs and scrollback settings for active command output.

## Design notes

The classic command-line interface remains supported. The UI command is an additional front end over the same workflow so developers can choose direct commands, interactive terminal use, or CI-friendly non-interactive flags.

## Documentation

See [Building terminal user interfaces](https://fission.rs/docs/guides/terminal-user-interfaces/) and the CLI reference at [fission.rs](https://fission.rs/docs/reference/cli/overview/).

## License

MIT
