use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::Env;
use std::collections::HashMap;

fn load_bundle(locale: &str, yaml: &str) -> anyhow::Result<TranslationBundle> {
    Ok(TranslationBundle {
        locale: Locale::from(locale),
        messages: serde_yaml::from_str::<HashMap<String, String>>(yaml)?,
    })
}

pub(crate) fn create_env() -> anyhow::Result<Env> {
    let mut env = Env::default();
    for bundle in inbox_example::translation_bundles() {
        env.i18n.add_bundle(bundle);
    }
    env.i18n
        .add_bundle(load_bundle("en-US", include_str!("../i18n/en-US.yaml"))?);
    env.i18n
        .add_bundle(load_bundle("es-ES", include_str!("../i18n/es-ES.yaml"))?);
    env.locale = Locale::from("en-US");
    Ok(env)
}

pub(crate) fn message(env: &Env, key: &str) -> String {
    env.i18n.get(&env.locale, key).unwrap_or(key).to_string()
}
