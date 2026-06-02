use fission_core::internal::BuildCtx;
use fission_core::ui::Widget;
use fission_core::{build, GlobalState, View};
use fission_widgets::TimePicker;

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

fn build_time_picker(hour: u32, minute: u32) -> Widget {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );

    build::enter(&mut ctx, &view, || {
        TimePicker {
            hour,
            minute,
            on_change: None,
        }
        .into()
    })
}

fn collect_text_inputs<'a>(node: &'a Widget, out: &mut Vec<&'a fission_core::ui::TextInput>) {
    if let Some(input) = fission_core::internal::widget_as_text_input(node) {
        out.push(input);
    }
    if let Some(row) = fission_core::internal::widget_as_row(node) {
        for child in &row.children {
            collect_text_inputs(child, out);
        }
    }
    if let Some(col) = fission_core::internal::widget_as_column(node) {
        for child in &col.children {
            collect_text_inputs(child, out);
        }
    }
    if let Some(button) = fission_core::internal::widget_as_button(node) {
        if let Some(child) = &button.child {
            collect_text_inputs(child, out);
        }
    }
    if let Some(container) = fission_core::internal::widget_as_container(node) {
        if let Some(child) = &container.child {
            collect_text_inputs(child, out);
        }
    }
}

fn collect_buttons<'a>(node: &'a Widget, out: &mut Vec<&'a fission_core::ui::Button>) {
    if let Some(button) = fission_core::internal::widget_as_button(node) {
        out.push(button);
        if let Some(child) = &button.child {
            collect_buttons(child, out);
        }
    }
    if let Some(row) = fission_core::internal::widget_as_row(node) {
        for child in &row.children {
            collect_buttons(child, out);
        }
    }
    if let Some(col) = fission_core::internal::widget_as_column(node) {
        for child in &col.children {
            collect_buttons(child, out);
        }
    }
    if let Some(container) = fission_core::internal::widget_as_container(node) {
        if let Some(child) = &container.child {
            collect_buttons(child, out);
        }
    }
}

#[test]
fn time_picker_uses_compact_stepper_buttons() {
    let node = build_time_picker(9, 0);
    let row =
        fission_core::internal::widget_as_row(&node).expect("TimePicker should lower to a row");
    assert_eq!(
        row.children.len(),
        3,
        "expected time picker to have HH : MM children"
    );
    let mut buttons = Vec::new();
    collect_buttons(&node, &mut buttons);
    assert_eq!(
        buttons.len(),
        4,
        "expected time picker to expose four compact stepper buttons"
    );
    for button in buttons {
        assert!(
            button.width.unwrap_or_default() <= 32.0,
            "time picker stepper buttons should remain aligned to compact field controls"
        );
        assert!(
            button.height.unwrap_or_default() <= 32.0,
            "time picker stepper buttons should remain aligned to compact field controls"
        );
    }
}

#[test]
fn time_picker_displays_zero_padded_values() {
    let node = build_time_picker(9, 0);
    let mut inputs = Vec::new();
    collect_text_inputs(&node, &mut inputs);
    let values: Vec<_> = inputs.iter().map(|input| input.value.as_str()).collect();
    assert_eq!(values, vec!["09", "00"]);
}
