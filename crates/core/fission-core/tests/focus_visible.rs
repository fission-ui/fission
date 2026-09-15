//! Focus is drawn after keyboard navigation but not after a click.

use fission_core::{Runtime, TextEditSource, WidgetId};
use fission_ir::CoreIR;

#[test]
fn keyboard_focus_is_drawn_and_pointer_focus_is_not() {
    let mut runtime = Runtime::default();
    let ir = CoreIR::default();
    let first = WidgetId::explicit("focus.first");
    let second = WidgetId::explicit("focus.second");

    runtime
        .set_focused_widget(&ir, Some(first), TextEditSource::Pointer)
        .unwrap();
    assert!(runtime.runtime_state.interaction.is_focused(first));
    assert!(!runtime.runtime_state.interaction.is_focus_visible(first));

    runtime
        .set_focused_widget(&ir, Some(second), TextEditSource::Keyboard)
        .unwrap();
    assert!(runtime.runtime_state.interaction.is_focus_visible(second));

    runtime
        .set_focused_widget(&ir, Some(first), TextEditSource::Programmatic)
        .unwrap();
    assert!(
        runtime.runtime_state.interaction.is_focus_visible(first),
        "programmatic focus keeps the modality the user was last using"
    );
}

#[test]
fn a_press_hides_focus_and_keyboard_navigation_shows_it_without_moving_focus() {
    use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
    use fission_layout::{LayoutPoint, LayoutSize, LayoutSnapshot};

    let mut runtime = Runtime::default();
    let ir = CoreIR::default();
    let layout = LayoutSnapshot::new(LayoutSize::new(400.0, 300.0));
    let focused = WidgetId::explicit("focus.focused");
    runtime
        .set_focused_widget(&ir, Some(focused), TextEditSource::Keyboard)
        .unwrap();
    assert!(runtime.runtime_state.interaction.focus_visible);

    runtime
        .handle_input(
            InputEvent::Pointer(PointerEvent::Down {
                pointer_id: Default::default(),
                kind: Default::default(),
                point: LayoutPoint::new(10.0, 10.0),
                button: PointerButton::Primary,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .unwrap();
    assert!(
        !runtime.runtime_state.interaction.focus_visible,
        "a press hides focus rings even when focus does not move"
    );

    runtime
        .handle_input(
            InputEvent::Keyboard(KeyEvent::Down {
                key_code: KeyCode::Tab,
                modifiers: 0,
            }),
            &ir,
            &layout,
        )
        .unwrap();
    assert!(
        runtime.runtime_state.interaction.focus_visible,
        "keyboard navigation shows focus rings again"
    );
}
