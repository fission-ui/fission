use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::Env;
use std::collections::HashMap;

fn bundle(locale: &str, source: &str) -> TranslationBundle {
    TranslationBundle {
        locale: Locale::from(locale),
        messages: serde_json::from_str::<HashMap<String, String>>(source)
            .expect("checked-in documentation translations must be valid JSON"),
    }
}

pub fn environment() -> Env {
    let mut env = Env::default();
    env.i18n
        .add_bundle(bundle("en-GB", include_str!("../i18n/en-GB.json")));
    env.i18n
        .add_bundle(bundle("es-ES", include_str!("../i18n/es-ES.json")));
    env.locale = "en-GB".into();
    env
}
