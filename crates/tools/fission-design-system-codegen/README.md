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
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=design/dsp.json");
    println!("cargo:rerun-if-changed=design/tokens.json");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    fission_design_system_codegen::generate(fission_design_system_codegen::Config {
        dsp_path: manifest_dir.join("design/dsp.json"),
        out_file: "app_design_system.rs".into(),
        type_name: "AppDesignSystem".into(),
        crate_path: "fission::theme".into(),
    })
    .expect("failed to generate AppDesignSystem");
}
```

Include the generated file with `include!(concat!(env!("OUT_DIR"), "/app_design_system.rs"));`, then install the generated design system on the selected shell. Keep the codegen crate version aligned with the runtime `fission` version so generated component and token contracts match.

## Documentation

See [Design systems](https://fission.rs/docs/guides/design-system/) for the user-facing workflow.

## License

Apache-2.0
