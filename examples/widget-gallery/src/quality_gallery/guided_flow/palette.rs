//! The reference design's own colours. This page reproduces a fixed mock and is
//! checked against a golden image, so it keeps these instead of theme tokens.

use fission::prelude::*;

pub(super) const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}

pub(super) const INK: Color = rgba(7, 17, 63, 255);

pub(super) const MUTED: Color = rgba(88, 105, 165, 255);

pub(super) const BLUE: Color = rgba(16, 31, 240, 255);

pub(super) const LINE: Color = rgba(219, 227, 245, 255);

pub(super) const SOFT: Color = rgba(246, 248, 255, 255);

pub(super) const PALE_BLUE: Color = rgba(234, 243, 255, 255);

pub(super) const GREEN: Color = rgba(0, 173, 128, 255);
