# fission-command-process

Internal process-tree supervision used by long-running Fission CLI commands.

The public developer interface remains the `fission` binary published by the
`cargo-fission` crate.

## Responsibilities

`ProcessSession` owns child registration, cancellation and bounded cleanup for commands such as local servers, device runners and test sessions. `SupervisedChild` waits for one launched process while preserving enough context to terminate its process tree. Captured execution helpers collect bounded output for commands whose diagnostics must be returned to the caller.

This crate is an implementation boundary, not a general process-management API. Fission command crates use it so Ctrl-C, startup failure and parent shutdown follow one cleanup policy instead of leaving compilers, servers or platform tools behind.

Do not depend on it from application code. Install the public CLI instead:

```sh
cargo install cargo-fission
```

When extending a long-running command, register every spawned child with the active session and preserve the original failure context if cleanup also reports an error. Never use detached children to avoid the supervision contract.

## Licence

Apache-2.0.
