//! Gallery colours, read from the active theme's tokens.
//!
//! Every colour resolves against the theme in scope when a widget builds, so
//! the gallery follows light and dark modes and the showcase's design-system
//! picker instead of one fixed palette. Call these only while building.

use crate::state::AnimationGalleryState;
use fission::op::Color;
use fission::Env;

/// How much of an accent colour a soft tint mixes into the surface.
const SOFT_TINT_AMOUNT: f32 = 0.16;

/// A soft background tint that can be named in `const` data, such as widget
/// summaries, and resolved against the theme when the widget builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Primary,
    Success,
    Secondary,
}

impl Tint {
    pub fn color(self) -> Color {
        match self {
            Tint::Primary => primary_subtle(),
            Tint::Success => soft(success()),
            Tint::Secondary => soft(secondary()),
        }
    }
}

fn read(pick: impl FnOnce(&Env) -> Color) -> Color {
    let (_, view) = fission::build::current::<AnimationGalleryState>();
    pick(view.env())
}

/// Mixes a little of `accent` into the surface colour.
fn soft(accent: Color) -> Color {
    let surface = surface();
    let mix = |from: u8, to: u8| {
        let from = f32::from(from);
        (from + (f32::from(to) - from) * SOFT_TINT_AMOUNT).round() as u8
    };
    Color {
        r: mix(surface.r, accent.r),
        g: mix(surface.g, accent.g),
        b: mix(surface.b, accent.b),
        a: surface.a,
    }
}

pub fn background() -> Color {
    read(|env| env.theme.tokens.colors.background)
}

pub fn surface() -> Color {
    read(|env| env.theme.tokens.colors.surface)
}

pub fn surface_sunken() -> Color {
    read(|env| env.theme.tokens.colors.surface_sunken)
}

pub fn text_primary() -> Color {
    read(|env| env.theme.tokens.colors.text_primary)
}

pub fn text_secondary() -> Color {
    read(|env| env.theme.tokens.colors.text_secondary)
}

pub fn text_muted() -> Color {
    read(|env| env.theme.tokens.colors.text_muted)
}

pub fn border() -> Color {
    read(|env| env.theme.tokens.colors.border)
}

pub fn border_strong() -> Color {
    read(|env| env.theme.tokens.colors.border_strong)
}

pub fn primary() -> Color {
    read(|env| env.theme.tokens.colors.primary)
}

pub fn on_primary() -> Color {
    read(|env| env.theme.tokens.colors.on_primary)
}

pub fn primary_subtle() -> Color {
    read(|env| env.theme.tokens.colors.primary_subtle)
}

pub fn secondary() -> Color {
    read(|env| env.theme.tokens.colors.secondary)
}

pub fn success() -> Color {
    read(|env| env.theme.tokens.colors.success)
}

pub fn warning() -> Color {
    read(|env| env.theme.tokens.colors.warning)
}

pub fn info() -> Color {
    read(|env| env.theme.tokens.colors.info)
}
