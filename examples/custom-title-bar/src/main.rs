use fission::i18n::{Locale, TranslationBundle};
use fission::prelude::*;

mod app;
mod state;
mod title_bar;
mod window_button;

fn main() -> anyhow::Result<()> {
    let mut env = Env::default();
    for (locale, json) in [
        ("en-US", include_str!("../i18n/en-US.json")),
        ("es-ES", include_str!("../i18n/es-ES.json")),
    ] {
        env.i18n.add_bundle(TranslationBundle {
            locale: Locale::from(locale),
            messages: serde_json::from_str(json)?,
        });
    }
    DesktopApp::<state::AppState, _>::new(app::App)
        .with_env(env)
        .with_design_system::<FissionFluent2DesignSystem>(DesignMode::Light)
        .with_title("Custom title bar")
        .with_decorations(false)
        .run()
}
