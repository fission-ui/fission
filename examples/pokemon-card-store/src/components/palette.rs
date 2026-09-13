use fission::prelude::Color;

pub fn card_accent(rgb: (u8, u8, u8)) -> Color {
    Color {
        r: rgb.0,
        g: rgb.1,
        b: rgb.2,
        a: 255,
    }
}
