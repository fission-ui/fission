use fission::ir::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum UiThemeMode {
    Light,
    #[default]
    Dark,
}

impl UiThemeMode {
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct UiPalette {
    pub(crate) background: Color,
    pub(crate) surface: Color,
    pub(crate) raised: Color,
    pub(crate) subtle: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) muted: Color,
    pub(crate) accent: Color,
    pub(crate) accent_text: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
}

impl UiPalette {
    pub(crate) fn for_mode(mode: UiThemeMode) -> Self {
        match mode {
            UiThemeMode::Dark => Self {
                background: rgb(11, 18, 32),
                surface: rgb(17, 27, 46),
                raised: rgb(24, 37, 61),
                subtle: rgb(31, 45, 72),
                border: rgb(72, 91, 123),
                text: rgb(235, 241, 250),
                muted: rgb(166, 179, 199),
                accent: rgb(20, 126, 112),
                accent_text: rgb(244, 255, 253),
                success: rgb(40, 204, 137),
                warning: rgb(245, 159, 33),
                error: rgb(242, 91, 91),
            },
            UiThemeMode::Light => Self {
                background: rgb(244, 247, 251),
                surface: rgb(255, 255, 255),
                raised: rgb(235, 241, 248),
                subtle: rgb(224, 234, 244),
                border: rgb(174, 190, 211),
                text: rgb(21, 31, 47),
                muted: rgb(86, 101, 124),
                accent: rgb(16, 120, 106),
                accent_text: rgb(255, 255, 255),
                success: rgb(19, 139, 89),
                warning: rgb(180, 104, 13),
                error: rgb(197, 48, 48),
            },
        }
    }
}

pub(crate) const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
