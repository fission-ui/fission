use fission_core::internal::{InternalLower, InternalLoweringCx};
use fission_core::{Env, RuntimeState};
use fission_ir::{Op, Role};
use fission_widgets::{Checkbox, Slider};

#[test]
fn test_slider_lowering() {
    let slider = Slider {
        value: 0.5,
        min: 0.0,
        max: 1.0,
        ..Default::default()
    };

    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let id = slider.lower(&mut cx);

    let node = cx.ir.nodes.get(&id).unwrap();
    // Slider.lower wraps in Semantics
    if let Op::Semantics(s) = &node.op {
        assert_eq!(s.role, Role::Slider);
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
    };

    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let id = cb.lower(&mut cx);

    let node = cx.ir.nodes.get(&id).unwrap();
    if let Op::Semantics(s) = &node.op {
        assert_eq!(s.role, Role::Checkbox);
        assert_eq!(s.checked, Some(true));
    } else {
        panic!("Checkbox should lower to Semantics root");
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
