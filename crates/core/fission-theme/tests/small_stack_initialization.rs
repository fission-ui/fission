use fission_theme::{DesignMode, DesignSystem, FissionDefaultDesignSystem};

#[test]
fn default_theme_initializes_on_an_android_sized_stack() {
    std::thread::Builder::new()
        .name("small-stack-theme-init".into())
        .stack_size(1024 * 1024)
        .spawn(|| {
            let theme = FissionDefaultDesignSystem::theme_ref(DesignMode::Light);
            assert!(!theme.components.recipes.is_empty());
        })
        .expect("small-stack theme thread should start")
        .join()
        .expect("default theme should initialize without overflowing a 1 MiB stack");
}
