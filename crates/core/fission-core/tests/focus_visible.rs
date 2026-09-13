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
