use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::Env;
use std::collections::HashMap;

fn load(locale: &str, source: &str) -> anyhow::Result<TranslationBundle> {
    Ok(TranslationBundle {
        locale: Locale::from(locale),
        messages: serde_json::from_str::<HashMap<String, String>>(source)?,
    })
}

pub fn create_env() -> anyhow::Result<Env> {
    let mut env = Env::default();
    env.i18n
        .add_bundle(load("en-US", include_str!("../i18n/en-US.json"))?);
    env.i18n
        .add_bundle(load("es-ES", include_str!("../i18n/es-ES.json"))?);
    env.locale = Locale::from("en-US");
    Ok(env)
}
