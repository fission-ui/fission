# fission-design-system-codegen

Build-time Design System Package code generation for Fission.

`fission-design-system-codegen` reads Design System Package JSON and token files during a project build and emits Rust theme code. Applications use the generated Rust types at runtime, so JSON parsing does not sit on the rendering hot path.

## What it contains

- Build-script helpers for reading DSP-compatible JSON inputs.
- Rust source generation for design tokens, themes, component variants, state styles, typography, spacing, shadows, borders, radii, and data-visualization palettes.
- Integration points for app crates that want their own design system instead of the default Fission design system.

## Example

```rust,ignore
// build.rs
fn main() -> anyhow::Result<()> {
    fission_design_system_codegen::generate("design/dsp.json", "design/tokens.json")?;
    Ok(())
}
```

## Documentation

See [Design systems](https://fission.rs/docs/guides/design-system/) for the user-facing workflow.

## License

MIT
