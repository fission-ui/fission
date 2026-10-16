//! A selected tag is filled, and its label stays readable on that fill.
//!
//! The on state used to be a thin ring in the primary colour over a pale
//! tint, which next to an unselected pill read as nothing at all. It now takes
//! the whole primary colour pair, so this checks the fill is really painted
//! and that its rendered foreground clears WCAG AA in every design system and
//! both modes.

use fission_core::action::{ActionEnvelope, ActionId, GlobalState};
use fission_core::authoring::BuildCtx;
use fission_core::env::{Env, RuntimeState};
use fission_core::op::Fill;
use fission_core::ui::{Container, WidgetKind};
use fission_core::{build, View, Widget};
use fission_theme::{
    Color, ColorRole, DesignMode, DesignSystem, FissionCupertinoDesignSystem,
    FissionDefaultDesignSystem, FissionEmberDesignSystem, FissionFluent2DesignSystem,
    FissionGraphiteDesignSystem, FissionLiquidGlassDesignSystem,
    FissionMaterialDesign3DesignSystem, Theme,
};
use fission_widgets::Tag;

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn build_tag(env: &Env, selected: bool) -> Widget {
    let state = TestState;
    let runtime_state = RuntimeState::default();
    let view = View::new(&state, &runtime_state, env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    build::enter(&mut ctx, &view, || {
        Tag {
            label: "All".into(),
            on_close: None,
            on_press: Some(ActionEnvelope {
                id: ActionId::from_name("showcase.filter"),
                payload: b"null".to_vec(),
            }),
            selected,
        }
        .into()
    })
}

/// The pill inside the button a pressable tag wraps itself in.
fn pill(widget: &Widget) -> &Container {
    let button = match widget.kind() {
        WidgetKind::Button(button) => button,
        other => panic!("a pressable tag is a button, got {other:?}"),
    };
    button
        .child
        .as_ref()
        .and_then(fission_core::internal::widget_as_container)
        .expect("tag pill")
}

fn label_color(container: &Container) -> Option<Color> {
    let row = container
        .child
        .as_ref()
        .and_then(fission_core::internal::widget_as_row)
        .expect("tag row");
    row.children
        .iter()
        .find_map(|child| fission_core::internal::widget_as_text(child).and_then(|text| text.color))
}

fn luminance(color: Color) -> f64 {
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

fn contrast(a: Color, b: Color) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la > lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

const PRESETS: [(&str, fn(DesignMode) -> Theme); 7] = [
    ("Tidewater", FissionDefaultDesignSystem::theme),
    ("Graphite", FissionGraphiteDesignSystem::theme),
    ("Ember", FissionEmberDesignSystem::theme),
    ("Material 3", FissionMaterialDesign3DesignSystem::theme),
    ("Fluent 2", FissionFluent2DesignSystem::theme),
    ("Cupertino", FissionCupertinoDesignSystem::theme),
    ("Liquid Glass", FissionLiquidGlassDesignSystem::theme),
];

#[test]
fn a_selected_tag_paints_a_fill_not_only_a_ring() {
    let env = Env::default();
    let pair = env.theme.tokens.colors.pair(ColorRole::Primary);

    let selected = build_tag(&env, true);
    let selected_pill = pill(&selected);
    assert_eq!(
        selected_pill.background_fill,
        Some(Fill::Solid(pair.fill)),
        "a selected tag paints the primary fill"
    );
    assert!(
        contrast(
            pair.fill,
            label_color(selected_pill).expect("selected label colour")
        ) >= 4.5
    );

    let unselected = build_tag(&env, false);
    let unselected_pill = pill(&unselected);
    assert_ne!(
        unselected_pill.background_fill, selected_pill.background_fill,
        "the two states have to differ by more than their border"
    );
    assert_ne!(
        unselected_pill.border_color, selected_pill.border_color,
        "the selected border follows its own fill"
    );
}

#[test]
fn a_selected_tag_stays_readable_in_every_preset_and_mode() {
    // A tag uses 13px normal text, so the foreground must clear the 4.5:1 body
    // text threshold. Most presets can use `on_primary`; if a design system's
    // preferred pair does not clear that threshold the widget chooses the more
    // legible black/white foreground rather than silently applying the 3:1
    // large-text/control threshold to small text.
    for (name, theme) in PRESETS {
        for mode in [DesignMode::Light, DesignMode::Dark] {
            let mut env = Env::default();
            env.theme = theme(mode);
            let pair = env.theme.tokens.colors.pair(ColorRole::Primary);
            let widget = build_tag(&env, true);
            let container = pill(&widget);

            assert_eq!(
                container.background_fill,
                Some(Fill::Solid(pair.fill)),
                "{name} {mode:?}: the selected fill is the preset's primary"
            );
            let label = label_color(container).expect("tag label colour");
            let ratio = contrast(pair.fill, label);
            assert!(
                ratio >= 4.5,
                "{name} {mode:?}: selected tag label contrast is {ratio:.2}:1"
            );
        }
    }
}
