# Contributing to Fission

Thank you for your interest in contributing to Fission. This document explains
how to get started, what we expect from contributions, and how the review
process works.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Pull Request Guidelines](#pull-request-guidelines)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
By participating you agree to uphold its terms.

## Getting Started

1. **Fork** the repository on GitHub.
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/worka-ai/fission.git
   cd fission
   ```
3. **Create a branch** for your work:
   ```bash
   git checkout -b my-feature
   ```
4. Make your changes, commit, and push to your fork.
5. Open a **Pull Request** against the `main` branch.

## Development Setup

### Prerequisites

- **Rust** -- Install via [rustup](https://rustup.rs/). A recent nightly
  toolchain is recommended:
  ```bash
  rustup install nightly
  rustup default nightly
  ```
- **System dependencies** for GPU rendering (Vello / wgpu):
  - **macOS**: Xcode command-line tools (`xcode-select --install`).
  - **Linux**: Install `libwayland-dev`, `libxkbcommon-dev`, `libvulkan-dev`
    (package names vary by distro).
  - **Windows**: A Vulkan-capable GPU driver and the Windows SDK.

### Building

```bash
cargo build --workspace
```

### Running Examples

```bash
cargo run -p counter
cargo run -p widget-gallery
cargo run -p inbox
```

## Code Style

- **Format** all code with `rustfmt` before committing:
  ```bash
  cargo fmt --all
  ```
- **Lint** with `clippy`:
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  ```
- Follow idiomatic Rust conventions. Prefer explicit types over excessive
  inference when it aids readability.
- Use `///` doc comments on all public items.
- Keep functions short and focused. If a function exceeds roughly 60 lines,
  consider splitting it.

## Testing

Fission has several layers of testing. Please add or update tests as
appropriate for your change.

### Unit Tests

Located alongside the code they test (in `#[cfg(test)]` modules or in
`tests/` directories within each crate).

```bash
cargo test --workspace
```

### Integration Tests

Found in `crates/tools/fission-test/tests/` and in each crate's `tests/`
directory. These exercise cross-crate behavior such as layout, hit testing,
and widget rendering.

```bash
cargo test -p fission-test
```

### End-to-End Tests

The `fission-test-driver` crate provides a remote test driver that can
exercise a running application. E2E tests live alongside the examples and
shell crates.

### What We Expect

- All existing tests must pass (`cargo test --workspace`).
- New features should include unit tests at minimum.
- Bug fixes should include a regression test when feasible.
- Layout or rendering changes should include snapshot or geometry assertion
  tests.

## Submitting Changes

1. Make sure your branch is up to date with `main`:
   ```bash
   git fetch origin
   git rebase origin/main
   ```
2. Run the full check suite:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
3. Push your branch and open a Pull Request.

## Pull Request Guidelines

- **Keep PRs focused.** One logical change per PR. If you find an unrelated
  issue while working, open a separate PR for it.
- **Write a clear description.** Explain *what* your change does and *why*.
  Reference any related issues.
- **Include tests.** See the [Testing](#testing) section above.
- **Update documentation** if your change affects public API or behavior.
- **Be responsive to review feedback.** We aim to review PRs within a few
  business days. If changes are requested, please address them promptly or
  explain why you disagree.

### Commit Messages

- Use the imperative mood: "add feature" not "added feature".
- Keep the subject line under 72 characters.
- Include a blank line between the subject and body if a body is needed.
- Reference issues when applicable: `fixes #123`.

## Reporting Issues

Use the [GitHub issue tracker](https://github.com/user/fission/issues) to
report bugs or request features. Please use the provided issue templates:

- **Bug reports**: Include steps to reproduce, expected behavior, actual
  behavior, and your environment (OS, Rust version, GPU).
- **Feature requests**: Describe the problem you are trying to solve and the
  solution you envision.

Search existing issues before opening a new one to avoid duplicates.

## Architecture Notes for Contributors

Fission separates the **authoring layer** (widgets, macros, high-level API)
from the **core layer** (IR, layout, semantics, rendering). This is by design:

- **Adding a new widget?** Work in `crates/authoring/fission-widgets/`. You
  should not need to modify any core crate.
- **Changing layout behavior?** Work in `crates/core/fission-layout/` and add
  tests in `crates/tools/fission-test/`.
- **Adding a new IR op?** This is a rare, carefully reviewed change. Open an
  issue to discuss the design first.
- **Adding a new renderer backend?** Implement the trait in
  `crates/rendering/fission-render/` and add your backend under
  `crates/rendering/`.

Thank you for contributing to Fission.
