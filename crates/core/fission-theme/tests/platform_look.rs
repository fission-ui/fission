//! The native look maps each platform to its own design system; the unified
//! look is Fission's default everywhere.

use fission_theme::{
    DesignMode, DesignSystem, FissionCupertinoDesignSystem, FissionDefaultDesignSystem,
    FissionFluent2DesignSystem, FissionMaterialDesign3DesignSystem, HostPlatform, PlatformLook,
};

fn name(look: PlatformLook, platform: HostPlatform) -> String {
    look.theme_ref(platform, DesignMode::Light)
        .design_system
        .info
        .name
        .clone()
}

#[test]
fn the_unified_look_is_fissions_default_on_every_platform() {
    let default = FissionDefaultDesignSystem::info().name.clone();
    for platform in [
        HostPlatform::MacOs,
        HostPlatform::Ios,
        HostPlatform::Android,
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Web,
        HostPlatform::Other,
    ] {
        assert_eq!(
            name(PlatformLook::Unified, platform),
            default,
            "{platform:?}"
        );
    }
    assert_eq!(PlatformLook::default(), PlatformLook::Unified);
}

#[test]
fn the_native_look_follows_the_platform() {
    let cupertino = FissionCupertinoDesignSystem::info().name.clone();
    assert_eq!(name(PlatformLook::Native, HostPlatform::MacOs), cupertino);
    assert_eq!(name(PlatformLook::Native, HostPlatform::Ios), cupertino);
    assert_eq!(
        name(PlatformLook::Native, HostPlatform::Android),
        FissionMaterialDesign3DesignSystem::info().name
    );
    assert_eq!(
        name(PlatformLook::Native, HostPlatform::Windows),
        FissionFluent2DesignSystem::info().name
    );
    // Platforms without a bundled native design system keep Fission's look.
    for platform in [HostPlatform::Linux, HostPlatform::Web, HostPlatform::Other] {
        assert_eq!(
            name(PlatformLook::Native, platform),
            FissionDefaultDesignSystem::info().name
        );
    }
}

#[test]
fn the_native_look_keeps_the_requested_mode() {
    let dark = PlatformLook::Native.theme_ref(HostPlatform::Android, DesignMode::Dark);
    assert_eq!(dark.design_system.mode, DesignMode::Dark);
}
