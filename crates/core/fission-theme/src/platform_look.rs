//! Opting an app into its host platform's own look.
//!
//! Fission's default is one look everywhere, with native behaviour. An app
//! that should look like the platform it runs on picks
//! [`PlatformLook::Native`]: Cupertino on macOS and iOS, Material Design 3 on
//! Android, Fluent 2 on Windows. Other platforms keep Fission's own look.

use serde::{Deserialize, Serialize};

use crate::{
    DesignMode, DesignSystem, FissionCupertinoDesignSystem, FissionDefaultDesignSystem,
    FissionFluent2DesignSystem, FissionMaterialDesign3DesignSystem, PackagedFont, Theme,
};

/// The platform an app runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostPlatform {
    MacOs,
    Ios,
    Android,
    Windows,
    Linux,
    Web,
    Other,
}

impl HostPlatform {
    /// The platform this build targets.
    pub const fn current() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::Web
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "ios") {
            Self::Ios
        } else if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

impl Default for HostPlatform {
    fn default() -> Self {
        Self::current()
    }
}

/// Whether an app looks the same everywhere or like its host platform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlatformLook {
    /// Fission's own look on every platform, with native behaviour.
    #[default]
    Unified,
    /// The host platform's look where Fission ships one.
    Native,
}

impl PlatformLook {
    /// The theme for this look on `platform` in `mode`.
    pub fn theme_ref(self, platform: HostPlatform, mode: DesignMode) -> &'static Theme {
        match (self, platform) {
            (Self::Native, HostPlatform::MacOs | HostPlatform::Ios) => {
                FissionCupertinoDesignSystem::theme_ref(mode)
            }
            (Self::Native, HostPlatform::Android) => {
                FissionMaterialDesign3DesignSystem::theme_ref(mode)
            }
            (Self::Native, HostPlatform::Windows) => FissionFluent2DesignSystem::theme_ref(mode),
            _ => FissionDefaultDesignSystem::theme_ref(mode),
        }
    }

    /// An owned copy of [`Self::theme_ref`].
    pub fn theme(self, platform: HostPlatform, mode: DesignMode) -> Theme {
        self.theme_ref(platform, mode).clone()
    }

    /// Font faces the chosen design system packages, to register before the
    /// first frame.
    pub fn font_faces(self, platform: HostPlatform) -> &'static [PackagedFont] {
        match (self, platform) {
            (Self::Native, HostPlatform::MacOs | HostPlatform::Ios) => {
                FissionCupertinoDesignSystem::font_faces()
            }
            (Self::Native, HostPlatform::Android) => {
                FissionMaterialDesign3DesignSystem::font_faces()
            }
            (Self::Native, HostPlatform::Windows) => FissionFluent2DesignSystem::font_faces(),
            _ => FissionDefaultDesignSystem::font_faces(),
        }
    }
}
