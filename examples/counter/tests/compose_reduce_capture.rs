use fission::prelude::*;
use fission_test::{TestDriver, TestHarness};

fission_component! {
    RepeatedLocalPicker<()> {
        state { selected: Option<u64> = None }
    }
    view(picker, _state, _cx) =>
    Column(children: [
        Text(match picker.selected {
            Some(id) => format!("Selected {id}"),
            None => "Nothing selected".to_string(),
        }),
        for id in [7_u64, 9_u64] {
            Button(
                child: Text(format!("Credential {id}")),
                key: id,
                handler: "select-credential",
                on_press: reduce(credential_id: u64 = id) {
                    picker.selected = Some(credential_id);
                },
            )
        },
    ])
}

#[test]
fn repeated_child_payload_updates_parent_component_local_state() {
    let harness = TestHarness::new(()).with_root_widget(RepeatedLocalPicker::new());
    let mut driver = TestDriver::new(harness);
    driver.pump().expect("initial picker frame");
    driver.assert_text_visible("Nothing selected");

    driver
        .tap_text("Credential 9")
        .expect("select credential 9");
    driver.pump().expect("picker frame after selection");

    driver.assert_text_visible("Selected 9");
}
