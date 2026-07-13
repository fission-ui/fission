use fission_core::internal::{InternalLower, InternalLoweringCx};
use fission_core::ui::GestureDetector;
use fission_core::{Env, RuntimeState};
use fission_ir::{Op, Role};
use fission_widgets::{Checkbox, Radio, Slider, Spacer, Switch};

#[test]
fn test_slider_lowering() {
    let slider = Slider {
        value: 0.5,
        min: 0.0,
        max: 1.0,
        ..Default::default()
    }
    .semantics_identifier("settings.volume");

    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let id = slider.lower(&mut cx);

    let node = cx.ir.nodes.get(&id).unwrap();
    // Slider.lower wraps in Semantics
    if let Op::Semantics(s) = &node.op {
        assert_eq!(s.role, Role::Slider);
        assert_eq!(s.identifier.as_deref(), Some("settings.volume"));
        assert_eq!(s.current_value, Some(0.5));
        assert_eq!(s.min_value, Some(0.0));
        assert_eq!(s.max_value, Some(1.0));
        assert!(s.draggable);
    } else {
        panic!("Slider should lower to Semantics root");
    }
}

#[test]
fn test_checkbox_lowering() {
    let cb = Checkbox {
        checked: true,
        ..Default::default()
    }
    .semantics_identifier("settings.enabled");

    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let id = cb.lower(&mut cx);

    let node = cx.ir.nodes.get(&id).unwrap();
    if let Op::Semantics(s) = &node.op {
        assert_eq!(s.role, Role::Checkbox);
        assert_eq!(s.identifier.as_deref(), Some("settings.enabled"));
        assert_eq!(s.checked, Some(true));
    } else {
        panic!("Checkbox should lower to Semantics root");
    }
}

#[test]
fn test_radio_lowering_preserves_semantics_identifier() {
    let radio = Radio {
        checked: true,
        ..Default::default()
    }
    .semantics_identifier("choices.primary");

    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let id = radio.lower(&mut cx);

    let node = cx.ir.nodes.get(&id).unwrap();
    if let Op::Semantics(s) = &node.op {
        assert_eq!(s.role, Role::Checkbox);
        assert_eq!(s.identifier.as_deref(), Some("choices.primary"));
        assert_eq!(s.checked, Some(true));
        assert!(s.focusable);
    } else {
        panic!("Radio should lower to Semantics root");
    }
}

#[test]
fn test_switch_lowering_preserves_semantics_identifier() {
    let switch = Switch {
        checked: true,
        ..Default::default()
    }
    .semantics_identifier("settings.dark_mode");

    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let id = switch.lower(&mut cx);

    let node = cx.ir.nodes.get(&id).unwrap();
    if let Op::Semantics(s) = &node.op {
        assert_eq!(s.role, Role::Switch);
        assert_eq!(s.identifier.as_deref(), Some("settings.dark_mode"));
        assert_eq!(s.checked, Some(true));
        assert!(s.focusable);
    } else {
        panic!("Switch should lower to Semantics root");
    }
}

#[test]
fn test_gesture_detector_lowering_preserves_semantics_identifier() {
    let detector = GestureDetector {
        drag_payload: Some(vec![1, 2, 3]),
        child: Spacer::default().into(),
        ..Default::default()
    }
    .semantics_identifier("canvas.drag_handle");

    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let id = detector.lower(&mut cx);

    let node = cx.ir.nodes.get(&id).unwrap();
    if let Op::Semantics(s) = &node.op {
        assert_eq!(s.identifier.as_deref(), Some("canvas.drag_handle"));
        assert!(s.draggable);
        assert_eq!(s.drag_payload.as_deref(), Some([1, 2, 3].as_slice()));
    } else {
        panic!("GestureDetector should lower to Semantics root");
    }
}

#[test]
fn test_tabs_structure() {
    // Tabs builds a Widget tree before lowering to IR.
    // We need to build it first.
    // But widgets don't expose build easily without View?
    // We can use the `fission_core::Widget` tree value.
    // But we need `View` and `BuildCtx`.
    // We can mock them.
}
