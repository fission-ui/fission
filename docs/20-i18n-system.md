# Internationalization (i18n) System (v1)

This document defines the **v1 internationalization (i18n) system** for the framework.
The design prioritizes:
- deterministic behavior,
- library-owned translations (like Flutter),
- simple authoring with JSON files,
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

### 1.3 Simple Authoring
- Translations are authored as JSON files.
- No ICU or MessageFormat knowledge is required for v1.
- Developers can add translations incrementally.

---

## 2. Translation Assets

### 2.1 File Layout

Each crate may include translation files under an `i18n/` directory:

```
my_widget_crate/
  i18n/
    en.json
    fr.json
    de.json
```

Files are keyed by locale identifier.

### 2.2 JSON Format (v1)

Each file contains a flat map of keys to strings:

```json
{
  "my_widget.button.ok": "OK",
  "my_widget.button.cancel": "Cancel"
}
```

Rules:
- Keys must be globally unique (namespacing by crate is recommended).
- Values are plain UTF-8 strings.
- No pluralization or formatting logic in v1.

---

## 3. Bundles and Registries

### 3.1 Translation Bundle

At build time (or runtime in dev mode), JSON files are compiled into a **TranslationBundle**:

```rust
pub struct TranslationBundle {
    pub id: BundleId,
    pub locale: Locale,
    pub entries: BTreeMap<String, String>,
}
```

Properties:
- Entries are stored in deterministic key order.
- Bundles are immutable once loaded.

### 3.2 I18n Registry

An **I18nRegistry** merges multiple bundles:

```rust
pub struct I18nRegistry {
    bundles: Vec<TranslationBundle>,
}
```

Merge rules:
- Bundle order is explicit and deterministic.
- Later bundles may override earlier keys (e.g. app overrides library).
- Overrides are visible and inspectable.

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

1. exact match (e.g. `pl-PL`)
2. language-only fallback (e.g. `pl`)
3. default locale (usually `en`)
4. key itself (as a last-resort debug fallback)

This chain is deterministic and consistent across platforms.

---

## 5. Using Translations in Widgets

### 5.1 Literal vs Translated Text

Widgets may use:
- literal strings (no translation),
- or message keys resolved through i18n.

Example:

```rust
Text::literal("OK")

Text::msg("my_widget.button.ok")
```

### 5.2 Resolution Timing

Translation lookup occurs:
- during layout/build resolution,
- not during rendering,
- producing a resolved string stored in the snapshot.

This ensures:
- deterministic rendering,
- easy snapshot inspection,
- no runtime locale surprises.

---

## 6. Snapshots, Tests, and Tooling

### 6.1 Snapshots

Snapshots include:
- active locale,
- registry/bundle identifiers,
- resolved text values.

This makes i18n behavior reproducible and debuggable.

### 6.2 Testing

The test harness can:
- set locale explicitly,
- provide custom translation bundles,
- assert resolved text values.

Example (conceptual):

```rust
harness.set_locale("fr");
assert_eq!(find("ok_button").text(), "OK");
```

---

## 7. Versioning and Compatibility

### 7.1 Bundle Versioning

Translation bundles are versioned artifacts:
- changes to translations do not break binary compatibility,
- but should be tracked for snapshot updates.

### 7.2 Key Stability

Keys are part of the public contract:
- removing keys is a breaking change for dependents,
- adding keys is backwards compatible.

---

## 8. What v1 Explicitly Does Not Support

The following are **out of scope for v1**:
- pluralization rules,
- gender or selection logic,
- locale-aware number/date formatting,
- bidirectional text shaping.

These omissions are intentional to keep v1 simple and predictable.

---

## 9. Forward Compatibility (v2 Preview)

v1 is designed to evolve into v2 without breaking APIs.

Planned v2 extensions:
- plural categories (`one`, `few`, `many`, `other`),
- structured message templates,
- pinned locale data for formatting,
- optional ICU-compatible message syntax.

Because v1 treats messages as data and uses explicit registries,
these features can be added without changing widget APIs.

---

## 10. Summary

The v1 i18n system provides:
- Flutter-like, library-owned translation files,
- deterministic lookup and fallback,
- explicit locale control,
- excellent testability and CI behavior.

It deliberately avoids ICU complexity initially,
while leaving a clear path to full internationalization support.
