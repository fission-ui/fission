---
name: fission-web
description: Build or modify Rust Fission websites and interactive Web apps using locally bundled API guidance.
---
<!-- CLI {{CLI_VERSION}}; framework API {{FRAMEWORK_VERSION}}; guidance {{GUIDANCE_VERSION}}; schema {{SCHEMA_VERSION}} -->

# Build a Fission website

Start from the actual app entrypoint, framework dependency and configured
targets. Follow applicable project instructions and preserve the user's scope.
This bundle describes Fission {{FRAMEWORK_VERSION}} APIs; a CLI asset version
does not establish compatibility with a local checkout or unresolved dependency.
Use `fission skills check --project-dir <app> --json` to inspect that evidence.

Read only the references needed now:

- New website or target choice: [setup and targets](references/setup-targets.md).
- Widget composition, retained state or navigation:
  [widgets, state and routing](references/widgets-state-routing.md).
- Visual design, tokens, responsiveness or translations:
  [design and i18n](references/design-i18n.md).
- Builds, browser testing or visual review:
  [testing and review](references/testing-review.md).

Author UI through Fission widgets. Keep shared app behavior in its Rust core and
shell entrypoints thin. Compile the changed APIs, exercise the selected rendered
target and fix observed failures. For an existing app, trace the changed
screen/component before editing; target setup is conditional, not a reason to
reinitialize the app.

For capabilities outside this bundle, inspect installed command help and
matching framework source. Explain a missing capability and its effect on the
requested outcome. Do not invent flags, assistant tools or deployment workflows.
