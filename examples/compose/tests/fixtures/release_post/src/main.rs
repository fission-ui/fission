mod counter;

use counter::CheckInCounter;
use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::*;

fn create_env() -> anyhow::Result<Env> {
    let mut env = Env::default();
    for (locale, messages) in [
        ("en-US", include_str!("../i18n/en-US.json")),
        ("es-ES", include_str!("../i18n/es-ES.json")),
    ] {
        env.i18n.add_bundle(TranslationBundle {
            locale: Locale::from(locale),
            messages: serde_json::from_str(messages)?,
        });
    }
    env.locale = Locale::from("en-US");
    Ok(env)
}

fn main() -> anyhow::Result<()> {
    DesktopApp::<(), _>::new(CheckInCounter {
        label: TextContent::Key("check_in.record_arrival".into()),
    })
    .with_env(create_env()?)
    .run()
}
