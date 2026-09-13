//! Translations for every label in the app, embedded from `i18n/`.

use fission::i18n::{Locale, TranslationBundle};
use std::collections::HashMap;

const SOURCES: [(&str, &str); 2] = [
    ("en-US", include_str!("../i18n/en-US.yaml")),
    ("es-ES", include_str!("../i18n/es-ES.yaml")),
];

/// Returns one bundle per supported locale.
///
/// # Panics
///
/// Panics if a checked-in translation file is not a flat map of strings; the
/// tests below keep that from reaching a release.
pub fn translation_bundles() -> Vec<TranslationBundle> {
    SOURCES
        .iter()
        .map(|(locale, yaml)| TranslationBundle {
            locale: Locale::from(*locale),
            messages: parse(locale, yaml),
        })
        .collect()
}

fn parse(locale: &str, yaml: &str) -> HashMap<String, String> {
    serde_yaml::from_str(yaml)
        .unwrap_or_else(|error| panic!("i18n/{locale}.yaml is not a map of strings: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_locale_translates_the_same_keys() {
        let keys: Vec<BTreeSet<String>> = SOURCES
            .iter()
            .map(|(locale, yaml)| parse(locale, yaml).into_keys().collect())
            .collect();
        assert!(keys.iter().all(|set| set == &keys[0]));
    }
}
