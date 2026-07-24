use fission::prelude::Color;

pub const CANVAS: Color = Color {
    r: 12,
    g: 18,
    b: 32,
    a: 255,
};
pub const SURFACE: Color = Color {
    r: 15,
    g: 23,
    b: 42,
    a: 255,
};
pub const SURFACE_RAISED: Color = Color {
    r: 24,
    g: 35,
    b: 58,
    a: 255,
};
pub const HERO_SURFACE: Color = Color {
    r: 17,
    g: 24,
    b: 39,
    a: 255,
};
pub const TEXT_PRIMARY: Color = Color {
    r: 248,
    g: 250,
    b: 252,
    a: 255,
};
pub const TEXT_BODY: Color = Color {
    r: 203,
    g: 213,
    b: 225,
    a: 255,
};
pub const TEXT_MUTED: Color = Color {
    r: 148,
    g: 163,
    b: 184,
    a: 255,
};
pub const BORDER: Color = Color {
    r: 71,
    g: 85,
    b: 105,
    a: 255,
};
pub const BLUE: Color = Color {
    r: 96,
    g: 165,
    b: 250,
    a: 255,
};
pub const BLUE_TEXT: Color = Color {
    r: 147,
    g: 197,
    b: 253,
    a: 255,
};
pub const GREEN: Color = Color {
    r: 34,
    g: 197,
    b: 94,
    a: 255,
};
pub const PINK: Color = Color {
    r: 244,
    g: 114,
    b: 182,
    a: 255,
};
pub const ORANGE: Color = Color {
    r: 251,
    g: 146,
    b: 60,
    a: 255,
};
pub const AMBER: Color = Color {
    r: 251,
    g: 191,
    b: 36,
    a: 255,
};
pub const RED: Color = Color {
    r: 248,
    g: 113,
    b: 113,
    a: 255,
};

pub fn card_accent(rgb: (u8, u8, u8)) -> Color {
    Color {
        r: rgb.0,
        g: rgb.1,
        b: rgb.2,
        a: 255,
    }
}
