<!-- fission-cli-generated-agents:v2 -->
<!-- CLI {{CLI_VERSION}}; framework API {{FRAMEWORK_VERSION}}; guidance {{GUIDANCE_VERSION}}; schema {{SCHEMA_VERSION}} -->

# Fission App Guidelines

Immediately after `fission init`, read this file and any `AGENTS.fission.md`
reported by init, alongside applicable user/contributor instructions.
For a website or browser app, read [the bundled web router]({{SKILL_PATH}}).
Load only its references relevant to the requested task.

- Author application UI in Rust with Fission. Preserve generated `fission.toml`;
  use supported CLI target/scaffold commands to change configuration.
- Select the requested browser target explicitly. Prefer `static-site` for a
  website that needs no runtime app state; choose `web` for browser interaction
  and state. Other applications keep their requested targets.
- Compile, run, inspect rendered output, and fix the requested behavior using
  actual installed APIs. Check command help before assuming a capability.
- Inspect offline guidance health with
  `fission skills check --project-dir <app> --json`; the manifest records CLI
  assets separately from the project's Fission dependency. Use
  `fission skills update --project-dir <app>` to refresh safe managed assets.
  Resolve customization conflicts explicitly; a generated marker alone never
  authorizes overwriting instructions.
