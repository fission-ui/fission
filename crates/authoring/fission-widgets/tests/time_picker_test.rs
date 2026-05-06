use fission_core::ui::Node;
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_widgets::TimePicker;

#[derive(Default, Clone, Debug)]
struct State;
impl AppState for State {}

fn build_time_picker(hour: u32, minute: u32) -> Node {
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

    TimePicker {
        hour,
        minute,
        on_change: None,
    }
    .build(&mut ctx, &view)
}

fn collect_text_inputs<'a>(node: &'a Node, out: &mut Vec<&'a fission_core::ui::TextInput>) {
    match node {
        Node::TextInput(input) => out.push(input),
        Node::Row(row) => {
            for child in &row.children {
                collect_text_inputs(child, out);
            }
        }
        Node::Column(col) => {
            for child in &col.children {
                collect_text_inputs(child, out);
            }
        }
        Node::Button(button) => {
            if let Some(child) = &button.child {
                collect_text_inputs(child, out);
            }
        }
        Node::Container(container) => {
            if let Some(child) = &container.child {
                collect_text_inputs(child, out);
            }
        }
        _ => {}
    }
}

fn collect_buttons<'a>(node: &'a Node, out: &mut Vec<&'a fission_core::ui::Button>) {
    match node {
        Node::Button(button) => {
            out.push(button);
            if let Some(child) = &button.child {
                collect_buttons(child, out);
            }
        }
        Node::Row(row) => {
            for child in &row.children {
                collect_buttons(child, out);
            }
        }
        Node::Column(col) => {
            for child in &col.children {
                collect_buttons(child, out);
            }
        }
        Node::Container(container) => {
            if let Some(child) = &container.child {
                collect_buttons(child, out);
            }
        }
        _ => {}
    }
}

#[test]
fn time_picker_uses_compact_stepper_buttons() {
    let node = build_time_picker(9, 0);
    let Node::Row(row) = node else {
        panic!("TimePicker should lower to a row");
    };
    assert_eq!(
        row.children.len(),
        3,
        "expected time picker to have HH : MM children"
    );
    let mut buttons = Vec::new();
    let row_node = Node::Row(row.clone());
    collect_buttons(&row_node, &mut buttons);
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
