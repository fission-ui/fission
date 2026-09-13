//! Switches, checkboxes and radios ease between states unless the app turns
//! built-in widget motion off.

use fission_core::authoring::BuildCtx;
use fission_core::motion::MotionDeclarationKind;
use fission_core::ui::{Checkbox, Radio, Switch, Widget};
use fission_core::{build, Env, RuntimeState, View, WidgetId, WidgetMotion};

/// The motion properties a widget declares, by name.
fn declared(env: &Env, widget: impl FnOnce() -> Widget) -> Vec<String> {
    let runtime = RuntimeState::default();
    let view = View::new(&(), &runtime, env, None);
    let mut context = BuildCtx::<()>::new();
    let _: Widget = build::enter(&mut context, &view, widget);
    context
        .motion_declarations
        .iter()
        .flat_map(|declaration| match &declaration.kind {
            MotionDeclarationKind::Tracks { tracks } => tracks
                .iter()
                .map(|track| format!("{:?}", track.property))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn toggles_declare_state_transitions_by_default() {
    let env = Env::default();
    let checkbox = declared(&env, || {
        Checkbox {
            id: Some(WidgetId::explicit("motion.checkbox")),
            checked: true,
            ..Default::default()
        }
        .into()
    });
    for property in ["BackgroundColor", "BorderColor", "Opacity", "Scale"] {
        assert!(
            checkbox.iter().any(|name| name == property),
            "checkbox {property}: {checkbox:?}"
        );
    }

    let switch = declared(&env, || {
        Switch {
            id: Some(WidgetId::explicit("motion.switch")),
            checked: true,
            ..Default::default()
        }
        .into()
    });
    for property in ["BackgroundColor", "TranslateX"] {
        assert!(
            switch.iter().any(|name| name == property),
            "switch {property}: {switch:?}"
        );
    }

    let radio = declared(&env, || {
        Radio {
            id: Some(WidgetId::explicit("motion.radio")),
            checked: true,
            ..Default::default()
        }
        .into()
    });
    for property in ["BorderColor", "BorderWidth", "Opacity", "Scale"] {
        assert!(
            radio.iter().any(|name| name == property),
            "radio {property}: {radio:?}"
        );
    }
}

#[test]
fn toggles_declare_no_motion_when_widget_motion_is_off() {
    let env = Env {
        widget_motion: WidgetMotion::Off,
        ..Env::default()
    };
    assert!(declared(&env, || Checkbox::default().into()).is_empty());
    assert!(declared(&env, || Switch::default().into()).is_empty());
    assert!(declared(&env, || Radio::default().into()).is_empty());
}
