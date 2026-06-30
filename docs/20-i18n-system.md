# Internationalization (i18n) System (v1)

This document defines the **v1 internationalization (i18n) system** for the framework.
The design prioritizes:

- deterministic behavior,
- library-owned translations (like Flutter),
- simple authoring with YAML files embedded with `include_str!`,
- explicit locale control for testing and CI.

Advanced features (plural rules, locale-aware formatting) are intentionally deferred to v2 and described briefly at the end.

---

## 1. Design Goals

### 1.1 Library-Owned Translations

- Each widget or library crate can ship its own translations.
- Applications compose translations from multiple sources.
- No central monolithic translation file is required.

### 1.2 Determinism

- Locale selection is explicit.
- Fallback rules are fixed and documented.
- Translation bundles are pinned and versioned.
- No OS locale APIs are consulted at runtime.
- Translation assets are normally embedded at compile time with `include_str!`.

### 1.3 Simple Authoring

- Translations are authored as YAML files.
- No ICU or MessageFormat knowledge is required for v1.
- Developers can add translations incrementally.
- The documented default is a flat YAML map parsed into `TranslationBundle` values at startup.

---

## 2. Translation Assets

### 2.1 File Layout

Each crate may include translation files under an `i18n/` directory:

```text
my_widget_crate/
  i18n/
    en-US.yaml
    fr-FR.yaml
    de-DE.yaml
```

Files are keyed by locale identifier.

### 2.2 YAML Format (v1)

Each file contains a flat map of keys to strings:

```yaml
my_widget.button.ok: "OK"
my_widget.button.cancel: "Cancel"
```

Rules:

- Keys must be globally unique; namespacing by crate is recommended.
- Values are plain UTF-8 strings.
- No pluralization or formatting logic is required in v1.
- YAML is the recommended documented authoring format because it is readable and works with `include_str!` on every target.

---

## 3. Bundles and Registries

### 3.1 Translation Bundle

At startup, applications parse embedded YAML strings into a **TranslationBundle**:

```rust
use std::collections::HashMap;

use fission::i18n::{Locale, TranslationBundle};

fn load_bundle(locale: &str, raw_yaml: &str) -> anyhow::Result<TranslationBundle> {
    let messages: HashMap<String, String> = serde_yaml::from_str(raw_yaml)?;

    Ok(TranslationBundle {
        locale: Locale::from(locale),
        messages,
    })
}

let en = load_bundle("en-US", include_str!("../i18n/en-US.yaml"))?;
```

Properties:

- Bundles are immutable once loaded.
- Bundles are built from checked-in translation files, so tests and releases use the same data.
- `include_str!` embeds the source text at compile time, avoiding runtime filesystem or network dependencies for first render.

### 3.2 I18n Registry

An **I18nRegistry** stores multiple bundles on `Env`:

```rust
let mut env = Env::default();
env.i18n.add_bundle(load_bundle(
    "en-US",
    include_str!("../i18n/en-US.yaml"),
)?);
env.i18n.add_bundle(load_bundle(
    "fr-FR",
    include_str!("../i18n/fr-FR.yaml"),
)?);
```

Merge rules:

- Bundle order is explicit and deterministic.
- Later bundles may override earlier keys when an application intentionally overrides library text.
- Overrides should be visible and inspectable in diagnostics.

---

## 4. Locale and Fallback Resolution

### 4.1 Explicit Locale

The active locale is explicit runtime input:

```rust
pub struct I18nContext {
    pub locale: Locale,
    pub registry_id: RegistryId,
}
```

Tests and apps must specify the locale explicitly.

### 4.2 Fallback Chain

Resolution follows a fixed chain:

1. exact match (e.g. `pl-PL`),
2. language-only fallback (e.g. `pl`),
3. default locale (usually `en-US`),
4. key itself or a visible missing-key marker as a debug fallback.

This chain is deterministic and consistent across platforms.

---

## 5. Using Translations in Widgets

### 5.1 Literal vs Translated Text

Widgets may use:

- literal strings for already-translated text, user data, file names, ids, and counts,
- message keys resolved through i18n for product copy.

Example:

```rust
Text::new(TextContent::Literal("OK".into()))

Text::new(TextContent::Key("my_widget.button.ok".into()))
```

### 5.2 Resolution Timing

Translation lookup occurs during widget build/lowering, before platform rendering consumes the text.

This ensures:

- deterministic rendering,
- easy snapshot inspection,
- no runtime locale surprises.

---

## 6. Env Integration

Translation bundles belong in `Env` because they are app-wide presentation context.

A shell should seed the initial environment with loaded bundles:

```rust
DesktopApp::new(MyApp)
    .with_env(create_env()?)
    .with_sync_env(|state: &MyState, env: &mut Env| {
        env.locale = state.locale.clone();
    })
    .run()?;
```

Keep the current locale in app state when the user can change it, then mirror that value into `env.locale` with `.with_sync_env(...)`.

---

## 7. Snapshots, Tests, and Tooling

### 7.1 Snapshots

Snapshots include:

- active locale,
- registry/bundle identifiers,
- resolved text values.

This makes i18n behavior reproducible and debuggable.

### 7.2 Testing

The test harness can:

- set locale explicitly,
- provide custom translation bundles,
- assert resolved text values.

Example (conceptual):

```rust
harness.set_locale("fr-FR");
assert_eq!(find("ok_button").text(), "OK");
```

---

## 8. Versioning and Compatibility

### 8.1 Bundle Versioning

Translation bundles are versioned artifacts:

- changes to translations do not break binary compatibility,
- but should be tracked for snapshot updates.

### 8.2 Key Stability

Keys are part of the public contract:

- removing keys is a breaking change for dependents,
- adding keys is backwards compatible.

---

## 9. What v1 Explicitly Does Not Support

The following are **out of scope for v1**:

- pluralization rules,
- gender or selection logic,
- locale-aware number/date formatting,
- bidirectional text shaping.

These omissions are intentional to keep v1 simple and predictable.

---

## 10. Forward Compatibility (v2 Preview)

v1 is designed to evolve into v2 without breaking APIs.

Planned v2 extensions:

- plural categories (`one`, `few`, `many`, `other`),
- structured message templates,
- pinned locale data for formatting,
- optional ICU-compatible message syntax.

Because v1 treats messages as data and uses explicit registries, these features can be added without changing widget APIs.

---

## 11. Summary

The v1 i18n system provides:

- Flutter-like, library-owned translation files,
- YAML plus `include_str!` as the recommended authoring and loading path,
- deterministic lookup and fallback,
- explicit locale control,
- excellent testability and CI behavior.

It deliberately avoids ICU complexity initially, while leaving a clear path to full internationalization support.
