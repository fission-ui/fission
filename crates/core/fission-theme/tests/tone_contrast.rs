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
        themes.push(("tidewater", FissionDefaultDesignSystem::theme(mode)));
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

/// A loading placeholder must stand apart from the page and from the card it
/// sits on. The sunken surface is darker than the page in a dark theme, so a
/// skeleton painted with it vanished; the placeholder role steps toward the
/// text colour in both modes instead.
#[test]
fn skeleton_placeholders_stand_out_in_light_and_dark() {
    // Matches the light placeholders, which already read clearly on the page.
    const MIN_SEPARATION: f64 = 1.06;
    for (name, theme) in themes() {
        let mode = theme.design_system.mode;
        let recipe = theme.recipe(fission_theme::recipes::Skeleton);
        let Some(Fill::Solid(placeholder)) = recipe.base.background.clone() else {
            panic!("{name} {mode:?}: skeleton needs a solid placeholder fill");
        };
        let colors = &theme.tokens.colors;
        for (surface, color) in [
            ("background", colors.background),
            ("surface", colors.surface),
        ] {
            let ratio = contrast(placeholder, color);
            assert!(
                ratio >= MIN_SEPARATION,
                "{name} {mode:?}: skeleton on {surface} is {ratio:.2}:1"
            );
        }
        if mode == DesignMode::Dark {
            assert!(
                luminance(placeholder) > luminance(colors.background),
                "{name} dark: a placeholder darker than the page reads as a hole"
            );
        }
    }
}

/// Every paired role's foreground is readable on its fill (WCAG AA, 4.5:1), in
/// each bundled look and mode, whether the pair is declared or aliased.
#[test]
fn every_colour_pair_is_readable_in_light_and_dark() {
    use fission_theme::{ColorRole, FissionGraphiteDesignSystem};
    const ROLES: [ColorRole; 9] = [
        ColorRole::Background,
        ColorRole::Surface,
        ColorRole::Card,
        ColorRole::Popover,
        ColorRole::Muted,
        ColorRole::Primary,
        ColorRole::Secondary,
        ColorRole::Accent,
        ColorRole::Destructive,
    ];
    let mut looks = themes();
    for mode in [DesignMode::Light, DesignMode::Dark] {
        looks.push(("graphite", FissionGraphiteDesignSystem::theme(mode)));
    }
    let mut failures = Vec::new();
    for (name, theme) in looks {
        let mode = theme.design_system.mode;
        for role in ROLES {
            let pair = theme.tokens.colors.pair(role);
            let ratio = contrast(pair.fill, pair.on);
            if ratio < 4.5 {
                failures.push(format!("{name} {mode:?} {role:?}: {ratio:.2}:1"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "pairs below AA:\n{}",
        failures.join("\n")
    );
}
