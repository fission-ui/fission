//! Tinted status tones stay readable in both modes.
//!
//! Badge tones used to point at fixed palette colours, so in a dark theme a
//! pale badge glowed on the page. They now read mode-aware colour roles; this
//! checks every tone's label against its own background at WCAG AA (4.5:1).

use fission_theme::{
    BadgeTone, ComponentSize, DesignMode, DesignSystem, Fill, FissionDefaultDesignSystem,
    FissionEmberDesignSystem, Theme,
};

fn luminance(color: fission_theme::Color) -> f64 {
    let channel = |value: u8| {
        let c = f64::from(value) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

fn contrast(a: fission_theme::Color, b: fission_theme::Color) -> f64 {
    let (light, dark) = {
        let (la, lb) = (luminance(a), luminance(b));
        if la > lb {
            (la, lb)
        } else {
            (lb, la)
        }
    };
    (light + 0.05) / (dark + 0.05)
}

const TONES: [BadgeTone; 7] = [
    BadgeTone::Brand,
    BadgeTone::Gray,
    BadgeTone::Success,
    BadgeTone::Warning,
    BadgeTone::Error,
    BadgeTone::Blue,
    BadgeTone::Orange,
];

fn themes() -> Vec<(&'static str, Theme)> {
    let mut themes = Vec::new();
    for mode in [DesignMode::Light, DesignMode::Dark] {
        themes.push(("graphite", FissionDefaultDesignSystem::theme(mode)));
        themes.push(("ember", FissionEmberDesignSystem::theme(mode)));
    }
    themes
}

#[test]
fn every_badge_tone_is_readable_in_light_and_dark() {
    for (name, theme) in themes() {
        let mode = theme.design_system.mode;
        for tone in TONES {
            let style = theme.components.badge.resolve(tone, ComponentSize::Md);
            let Some(Fill::Solid(background)) = style.background.clone() else {
                panic!("{name} {mode:?} {tone:?} badge has no solid background");
            };
            let text = style
                .text_color
                .unwrap_or_else(|| panic!("{name} {mode:?} {tone:?} badge has no text colour"));
            let ratio = contrast(text, background);
            assert!(
                ratio >= 4.5,
                "{name} {mode:?} {tone:?} badge text contrast is {ratio:.2}:1"
            );
        }
    }
}

#[test]
fn dark_badges_sit_darker_than_their_label() {
    // A tinted badge on a dark page is a dark tint with a light label, not a
    // pale pill carried over from the light theme.
    for mode_theme in [
        FissionDefaultDesignSystem::theme(DesignMode::Dark),
        FissionEmberDesignSystem::theme(DesignMode::Dark),
    ] {
        for tone in TONES {
            let style = mode_theme.components.badge.resolve(tone, ComponentSize::Md);
            let Some(Fill::Solid(background)) = style.background.clone() else {
                continue;
            };
            let text = style.text_color.expect("badge text colour");
            assert!(
                luminance(background) < luminance(text),
                "{tone:?} dark badge background is lighter than its label"
            );
        }
    }
}
