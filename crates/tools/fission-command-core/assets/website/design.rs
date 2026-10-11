use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::*;
use std::collections::HashMap;

#[allow(unused_mut)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/website_design_system.rs"));
}
pub use generated::WebsiteDesignSystem;

// These are layout constraints, not theme values. Visual tokens live in design/.
pub const NARROW_WIDTH: f32 = 720.0;
pub const CONTENT_WIDTH: f32 = 1120.0;

pub fn environment() -> anyhow::Result<Env> {
    let mut env = Env::default();
    env.theme = WebsiteDesignSystem::theme(DesignMode::Light);
    env.locale = Locale::from("en");
    for (locale, source) in [
        ("en", include_str!("../i18n/en.json")),
        ("es", include_str!("../i18n/es.json")),
    ] {
        env.i18n.add_bundle(TranslationBundle {
            locale: Locale::from(locale),
            messages: serde_json::from_str::<HashMap<String, String>>(source)?,
        });
    }
    Ok(env)
}

pub fn message(key: &str) -> TextContent {
    TextContent::Key(key.into())
}
