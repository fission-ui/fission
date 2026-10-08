<!-- CLI {{CLI_VERSION}}; framework API {{FRAMEWORK_VERSION}}; guidance {{GUIDANCE_VERSION}}; schema {{SCHEMA_VERSION}} -->

# Design and translations

Use clear hierarchy, purposeful spacing and responsive mobile/desktop layouts.
Progressively reveal advanced choices. Trace visual findings to widget code and
inspect related spacing, overflow, typography and state in the same region.
Use truthful progress, useful empty states and editable defaults. Product growth
and sharing features are optional when they serve the requested outcome.

Prefer theme/design-system tokens over scattered hard-coded visual values:

```rust
let (_, view) = fission::build::current::<AppState>();
let tokens = &view.env().theme.tokens;
let panel: Widget = Container::new(Text::new("Dashboard"))
    .bg(tokens.colors.background).padding_all(tokens.spacing.xl).into();
```

Inspect token names in matching framework/theme output. Literal values are
acceptable for narrow one-off geometry. For a custom design system maintain
checked-in DSP JSON/tokens, generate a Rust type using version-aligned
`fission-design-system-codegen` as a build dependency:

```rust
fn main() {
    println!("cargo:rerun-if-changed=design/dsp.json");
    println!("cargo:rerun-if-changed=design/tokens.json");
    let dsp = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("design/dsp.json");
    fission_design_system_codegen::generate(
        fission_design_system_codegen::Config::new(dsp)
            .out_file("app_design_system.rs")
            .type_name("AppDesignSystem")
            .crate_path("fission::theme"),
    ).expect("generate design system");
}
```

Include with `include!(concat!(env!("OUT_DIR"), "/app_design_system.rs"))`.
Install with `.with_design_system::<AppDesignSystem>(DesignMode::Light)`
on the shell/site builder. Default DSP files are in matching source at
`crates/core/fission-theme/design/default/`; obtain them only when needed and
keep project copies rather than parsing upstream at runtime.
DSP font paths resolve relative to the DSP file. When copying the defaults,
also copy the fonts they reference into the project and adjust those relative
paths (for example, `../../fonts/Inter/...` to `fonts/Inter/...` under
`design/`). Codegen validates the files and embeds them; copying only the two
JSON files is insufficient. Use `Config::new` and its builder methods because
`Config` is non-exhaustive.

Use stable meaning-based translation keys and checked-in translation files for
localized products. Add the languages the product requires; this guide imposes
no extra locales. Direct API, independent of file format:

```rust
use std::collections::HashMap;
use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::*;
let mut env = Env::default();
env.i18n.add_bundle(TranslationBundle {
    locale: Locale::from("en-US"),
    messages: HashMap::from([("home.title".into(), "Welcome".into())]),
});
let title = Text::new(TextContent::Key("home.title".into()));
```

Keep user-selected theme/locale in `GlobalState`, then mirror presentation
inputs with `.with_sync_env(|state: &AppState, env: &mut Env| { ... })`.
That hook must not launch I/O/jobs or update domain data. Static builds seed
bundles through `FissionSite::with_env` or `translation_bundle`.
Check long translated strings and locale-specific routes when present.
